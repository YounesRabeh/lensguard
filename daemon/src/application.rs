use std::sync::{Arc, Mutex};

use camera_app_resolver::{ApplicationResolver, ResolutionRequest};
use camera_core::{ApplicationIdentity, MonitorEvent, MonitorState};
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::debug;

/// Application-identity boundary used by the asynchronous event processor.
pub trait IdentityResolver: Send + 'static {
    fn resolve(&mut self, request: ResolutionRequest) -> ApplicationIdentity;
}

impl IdentityResolver for ApplicationResolver {
    fn resolve(&mut self, request: ResolutionRequest) -> ApplicationIdentity {
        ApplicationResolver::resolve(self, request)
    }
}

/// Application pipeline failure.
#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("application resolver worker failed: {0}")]
    ResolverTask(#[from] tokio::task::JoinError),
    #[error("application resolver lock was poisoned")]
    ResolverPoisoned,
    #[error("D-Bus publication task ended before the application pipeline")]
    PublisherClosed,
}

/// Resolves, reduces, reconciles, and forwards backend events in arrival order.
///
/// # Errors
///
/// Returns an error if the blocking resolver task fails, its lock is poisoned, or the bounded
/// publication channel closes before all accepted events are forwarded.
pub async fn run_application<R>(
    mut incoming: mpsc::Receiver<MonitorEvent>,
    outgoing: mpsc::Sender<MonitorEvent>,
    resolver: R,
    mut state: MonitorState,
) -> Result<MonitorState, ApplicationError>
where
    R: IdentityResolver,
{
    let resolver = Arc::new(Mutex::new(resolver));
    while let Some(event) = incoming.recv().await {
        let event = resolve_event(event, Arc::clone(&resolver)).await?;
        for effective_event in reduce_and_reconcile(&mut state, event) {
            outgoing
                .send(effective_event)
                .await
                .map_err(|_| ApplicationError::PublisherClosed)?;
        }
    }
    debug!("backend event channel drained");
    Ok(state)
}

async fn resolve_event<R>(
    event: MonitorEvent,
    resolver: Arc<Mutex<R>>,
) -> Result<MonitorEvent, ApplicationError>
where
    R: IdentityResolver,
{
    match event {
        MonitorEvent::SessionStarted(mut session) => {
            let request = ResolutionRequest::from(&session.application);
            session.application = tokio::task::spawn_blocking(move || {
                let mut resolver = resolver
                    .lock()
                    .map_err(|_| ApplicationError::ResolverPoisoned)?;
                Ok::<_, ApplicationError>(resolver.resolve(request))
            })
            .await??;
            Ok(MonitorEvent::SessionStarted(session))
        }
        MonitorEvent::SessionUpdated(mut session) => {
            let request = ResolutionRequest::from(&session.application);
            session.application = tokio::task::spawn_blocking(move || {
                let mut resolver = resolver
                    .lock()
                    .map_err(|_| ApplicationError::ResolverPoisoned)?;
                Ok::<_, ApplicationError>(resolver.resolve(request))
            })
            .await??;
            Ok(MonitorEvent::SessionUpdated(session))
        }
        other => Ok(other),
    }
}

fn reduce_and_reconcile(state: &mut MonitorState, event: MonitorEvent) -> Vec<MonitorEvent> {
    let mut effective_events = Vec::new();
    if matches!(event, MonitorEvent::BackendUnavailable { .. }) {
        for session in state.snapshot().active_sessions {
            let stop = MonitorEvent::SessionStopped(session.id);
            if state.apply(stop.clone()) {
                effective_events.push(stop);
            }
        }
    }
    if state.apply(event.clone()) {
        effective_events.push(event);
    }
    effective_events
}

#[cfg(test)]
mod tests {
    use camera_app_resolver::ResolutionRequest;
    use camera_core::{
        ApplicationIdentity, CameraDevice, CameraSession, DetectionBackend, DeviceId, MonitorEvent,
        MonitorState, SessionId,
    };
    use tokio::sync::mpsc;

    use super::{IdentityResolver, run_application};

    struct FakeResolver;

    impl IdentityResolver for FakeResolver {
        fn resolve(&mut self, request: ResolutionRequest) -> ApplicationIdentity {
            ApplicationIdentity {
                pid: request.pid,
                app_id: Some(String::from("org.example.Resolved")),
                display_name: String::from("Resolved Camera"),
                binary: request.binary,
            }
        }
    }

    fn session() -> CameraSession {
        CameraSession {
            id: SessionId::new("session-one").unwrap(),
            application: ApplicationIdentity {
                pid: Some(42),
                app_id: None,
                display_name: String::from("Unknown application"),
                binary: Some(String::from("camera")),
            },
            device: CameraDevice {
                id: DeviceId::new("camera-one").unwrap(),
                display_name: String::from("Front Camera"),
                node_name: None,
            },
            started_at_unix_ms: 10,
            backend: DetectionBackend::PipeWire,
        }
    }

    #[tokio::test]
    async fn events_flow_through_resolution_state_and_publication() {
        let (incoming_tx, incoming_rx) = mpsc::channel(4);
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(4);
        let task = tokio::spawn(run_application(
            incoming_rx,
            outgoing_tx,
            FakeResolver,
            MonitorState::new(),
        ));

        incoming_tx
            .send(MonitorEvent::SessionStarted(session()))
            .await
            .unwrap();
        incoming_tx
            .send(MonitorEvent::BackendUnavailable {
                reason: String::from("fake disconnect"),
            })
            .await
            .unwrap();
        drop(incoming_tx);

        let start = outgoing_rx.recv().await.unwrap();
        let stop = outgoing_rx.recv().await.unwrap();
        let unavailable = outgoing_rx.recv().await.unwrap();
        let MonitorEvent::SessionStarted(start) = start else {
            panic!("expected a start event");
        };
        assert_eq!(start.application.display_name, "Resolved Camera");
        assert!(matches!(stop, MonitorEvent::SessionStopped(_)));
        assert!(matches!(
            unavailable,
            MonitorEvent::BackendUnavailable { .. }
        ));

        let final_state = task.await.unwrap().unwrap();
        assert!(!final_state.active());
        assert!(!final_state.backend_available());
    }
}
