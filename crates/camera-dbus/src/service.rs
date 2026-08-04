use camera_core::{MonitorEvent, MonitorSnapshot, MonitorState};
use zbus::{Connection, connection, interface, object_server::SignalEmitter};

use crate::{
    BUS_NAME, CameraMonitorError, OBJECT_PATH, PING_RESPONSE, ServiceError, SessionDto, VERSION,
    dto::snapshot_sessions,
};

struct CameraMonitorInterface {
    state: MonitorState,
    ping_response: &'static str,
    version: &'static str,
}

impl CameraMonitorInterface {
    fn new(state: MonitorState) -> Self {
        Self {
            state,
            ping_response: PING_RESPONSE,
            version: VERSION,
        }
    }

    fn snapshot(&self) -> MonitorSnapshot {
        self.state.snapshot()
    }
}

#[interface(name = "io.github.younesrabeh.CameraMonitor1")]
impl CameraMonitorInterface {
    // Keep a typed D-Bus error boundary even though the in-memory snapshot is currently infallible.
    #[allow(clippy::unnecessary_wraps)]
    fn get_active_sessions(&self) -> Result<Vec<SessionDto>, CameraMonitorError> {
        Ok(snapshot_sessions(&self.snapshot()))
    }

    // Keep a typed D-Bus error boundary so future health checks do not change the public method.
    #[allow(clippy::unnecessary_wraps)]
    fn ping(&self) -> Result<&str, CameraMonitorError> {
        Ok(self.ping_response)
    }

    #[zbus(property)]
    fn active(&self) -> bool {
        self.state.active()
    }

    #[zbus(property)]
    fn active_session_count(&self) -> u32 {
        u32::try_from(self.state.active_session_count()).unwrap_or(u32::MAX)
    }

    #[zbus(property)]
    fn backend_available(&self) -> bool {
        self.state.backend_available()
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn version(&self) -> &str {
        self.version
    }

    #[zbus(signal)]
    async fn state_changed(
        emitter: &SignalEmitter<'_>,
        active: bool,
        active_session_count: u32,
        backend_available: bool,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn session_started(emitter: &SignalEmitter<'_>, session: SessionDto) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn session_stopped(emitter: &SignalEmitter<'_>, session_id: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn backend_availability_changed(
        emitter: &SignalEmitter<'_>,
        available: bool,
    ) -> zbus::Result<()>;
}

/// Running D-Bus object and its owned bus connection.
pub struct DbusService {
    connection: Connection,
}

impl DbusService {
    /// Starts the service on the current user session bus.
    ///
    /// # Errors
    ///
    /// Returns [`ServiceError`] when the session bus cannot be reached, the object cannot be
    /// exported, or the well-known name cannot be acquired.
    pub async fn session(initial_state: MonitorState) -> Result<Self, ServiceError> {
        let connection = connection::Builder::session()?
            .serve_at(OBJECT_PATH, CameraMonitorInterface::new(initial_state))?
            .name(BUS_NAME)?
            .build()
            .await?;
        Ok(Self { connection })
    }

    /// Starts the service on an existing bus connection, primarily for isolated integration tests.
    ///
    /// # Errors
    ///
    /// Returns [`ServiceError`] when the object cannot be exported or the name cannot be acquired.
    pub async fn on_connection(
        connection: Connection,
        initial_state: MonitorState,
    ) -> Result<Self, ServiceError> {
        connection
            .object_server()
            .at(OBJECT_PATH, CameraMonitorInterface::new(initial_state))
            .await?;
        connection.request_name(BUS_NAME).await?;
        Ok(Self { connection })
    }

    /// Applies one domain event and emits all corresponding D-Bus notifications.
    ///
    /// Returns `true` only when observable state changed.
    ///
    /// # Errors
    ///
    /// Returns [`ServiceError`] if the exported object cannot be accessed or a signal cannot be
    /// sent.
    pub async fn apply_event(&self, event: MonitorEvent) -> Result<bool, ServiceError> {
        let interface_ref = self
            .connection
            .object_server()
            .interface::<_, CameraMonitorInterface>(OBJECT_PATH)
            .await?;
        let mut interface = interface_ref.get_mut().await;
        let before = interface.snapshot();
        if !interface.state.apply(event.clone()) {
            return Ok(false);
        }
        let after = interface.snapshot();
        let emitter = interface_ref.signal_emitter();

        if before.active() != after.active() {
            interface.active_changed(emitter).await?;
        }
        if before.active_session_count() != after.active_session_count() {
            interface.active_session_count_changed(emitter).await?;
        }
        if before.backend_available != after.backend_available {
            interface.backend_available_changed(emitter).await?;
            CameraMonitorInterface::backend_availability_changed(emitter, after.backend_available)
                .await?;
        }

        CameraMonitorInterface::state_changed(
            emitter,
            after.active(),
            session_count(&after),
            after.backend_available,
        )
        .await?;
        match event {
            MonitorEvent::SessionStarted(session) => {
                CameraMonitorInterface::session_started(emitter, SessionDto::from(&session))
                    .await?;
            }
            MonitorEvent::SessionStopped(session_id) => {
                CameraMonitorInterface::session_stopped(emitter, session_id.as_str()).await?;
            }
            MonitorEvent::SessionUpdated(_)
            | MonitorEvent::BackendUnavailable { .. }
            | MonitorEvent::BackendRecovered => {}
        }

        Ok(true)
    }

    /// Returns the current transport-independent state for diagnostics and tests.
    ///
    /// # Errors
    ///
    /// Returns [`ServiceError`] when the exported interface is unavailable.
    pub async fn snapshot(&self) -> Result<MonitorSnapshot, ServiceError> {
        let interface_ref = self
            .connection
            .object_server()
            .interface::<_, CameraMonitorInterface>(OBJECT_PATH)
            .await?;
        let interface = interface_ref.get().await;
        Ok(interface.snapshot())
    }

    /// Returns the owned D-Bus connection.
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    /// Releases the well-known name and unregisters the object.
    ///
    /// # Errors
    ///
    /// Returns [`ServiceError`] if cleanup cannot be completed.
    pub async fn shutdown(self) -> Result<(), ServiceError> {
        self.connection.release_name(BUS_NAME).await?;
        self.connection
            .object_server()
            .remove::<CameraMonitorInterface, _>(OBJECT_PATH)
            .await?;
        Ok(())
    }
}

fn session_count(snapshot: &MonitorSnapshot) -> u32 {
    u32::try_from(snapshot.active_session_count()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use camera_core::{
        ApplicationIdentity, CameraDevice, CameraSession, DetectionBackend, DeviceId, MonitorEvent,
        MonitorState, SessionId,
    };

    use super::{CameraMonitorInterface, session_count};
    use crate::dto::snapshot_sessions;

    fn session(id: &str) -> CameraSession {
        CameraSession {
            id: SessionId::new(id).unwrap(),
            application: ApplicationIdentity {
                pid: None,
                app_id: None,
                display_name: format!("Application {id}"),
                binary: None,
            },
            device: CameraDevice {
                id: DeviceId::new(format!("device-{id}")).unwrap(),
                display_name: format!("Camera {id}"),
                node_name: None,
            },
            started_at_unix_ms: 100,
            backend: DetectionBackend::PipeWire,
        }
    }

    #[test]
    fn empty_single_and_multiple_snapshots_are_deterministic() {
        let mut state = MonitorState::new();
        let empty = CameraMonitorInterface::new(state.clone()).snapshot();
        assert!(!empty.active());
        assert_eq!(session_count(&empty), 0);
        assert!(snapshot_sessions(&empty).is_empty());

        state.apply(MonitorEvent::SessionStarted(session("z-last")));
        let single = CameraMonitorInterface::new(state.clone()).snapshot();
        assert!(single.active());
        assert_eq!(session_count(&single), 1);

        state.apply(MonitorEvent::SessionStarted(session("a-first")));
        let multiple = CameraMonitorInterface::new(state).snapshot();
        let ids = snapshot_sessions(&multiple)
            .into_iter()
            .map(|session| session.session_id)
            .collect::<Vec<_>>();
        assert_eq!(ids, ["a-first", "z-last"]);
    }
}
