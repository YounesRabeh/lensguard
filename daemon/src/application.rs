use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use camera_app_resolver::{ApplicationResolver, ResolutionRequest};
use camera_core::{ApplicationIdentity, MonitorEvent, MonitorState};
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{debug, warn};

/// Maximum time the event pipeline waits for process and desktop-entry metadata.
pub const DEFAULT_RESOLUTION_TIMEOUT: Duration = Duration::from_secs(1);

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
    incoming: mpsc::Receiver<MonitorEvent>,
    outgoing: mpsc::Sender<MonitorEvent>,
    resolver: R,
    state: MonitorState,
) -> Result<MonitorState, ApplicationError>
where
    R: IdentityResolver,
{
    run_application_with_timeout(
        incoming,
        outgoing,
        resolver,
        state,
        DEFAULT_RESOLUTION_TIMEOUT,
    )
    .await
}

/// Runs the application pipeline with an explicit metadata-resolution timeout.
///
/// This entry point exists for deterministic fault injection. Production uses
/// [`DEFAULT_RESOLUTION_TIMEOUT`]. If one blocking lookup exceeds the timeout, later events use
/// their existing `PipeWire` identity until that worker exits; this prevents detached worker
/// accumulation while preserving camera activity state.
///
/// # Errors
///
/// Returns the same resolver, publication, and worker failures as [`run_application`].
pub async fn run_application_with_timeout<R>(
    mut incoming: mpsc::Receiver<MonitorEvent>,
    outgoing: mpsc::Sender<MonitorEvent>,
    resolver: R,
    mut state: MonitorState,
    resolution_timeout: Duration,
) -> Result<MonitorState, ApplicationError>
where
    R: IdentityResolver,
{
    let resolver = Arc::new(Mutex::new(resolver));
    let resolver_busy = Arc::new(AtomicBool::new(false));
    while let Some(event) = incoming.recv().await {
        let event = resolve_event(
            event,
            Arc::clone(&resolver),
            Arc::clone(&resolver_busy),
            resolution_timeout,
        )
        .await?;
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
    resolver_busy: Arc<AtomicBool>,
    resolution_timeout: Duration,
) -> Result<MonitorEvent, ApplicationError>
where
    R: IdentityResolver,
{
    match event {
        MonitorEvent::SessionStarted(mut session) => {
            let request = ResolutionRequest::from(&session.application);
            session.application = resolve_identity(
                request,
                session.application,
                resolver,
                resolver_busy,
                resolution_timeout,
            )
            .await?;
            Ok(MonitorEvent::SessionStarted(session))
        }
        MonitorEvent::SessionUpdated(mut session) => {
            let request = ResolutionRequest::from(&session.application);
            session.application = resolve_identity(
                request,
                session.application,
                resolver,
                resolver_busy,
                resolution_timeout,
            )
            .await?;
            Ok(MonitorEvent::SessionUpdated(session))
        }
        other => Ok(other),
    }
}

struct ResolverBusyGuard(Arc<AtomicBool>);

impl Drop for ResolverBusyGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

async fn resolve_identity<R>(
    request: ResolutionRequest,
    fallback: ApplicationIdentity,
    resolver: Arc<Mutex<R>>,
    resolver_busy: Arc<AtomicBool>,
    resolution_timeout: Duration,
) -> Result<ApplicationIdentity, ApplicationError>
where
    R: IdentityResolver,
{
    if resolver_busy
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        debug!("metadata resolver is still busy; preserving backend identity");
        return Ok(fallback);
    }

    let busy_guard = ResolverBusyGuard(resolver_busy);
    let worker = tokio::task::spawn_blocking(move || {
        let _busy_guard = busy_guard;
        let mut resolver = resolver
            .lock()
            .map_err(|_| ApplicationError::ResolverPoisoned)?;
        Ok::<_, ApplicationError>(resolver.resolve(request))
    });

    Ok(
        if let Ok(result) = tokio::time::timeout(resolution_timeout, worker).await {
            result??
        } else {
            warn!(
                timeout_ms = resolution_timeout.as_millis(),
                "application metadata resolution timed out; preserving backend identity"
            );
            fallback
        },
    )
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
    use std::thread;
    use std::time::{Duration, Instant};

    use camera_app_resolver::ResolutionRequest;
    use camera_core::{
        ApplicationIdentity, CameraDevice, CameraSession, DetectionBackend, DeviceId, MonitorEvent,
        MonitorState, SessionId,
    };
    use tokio::sync::mpsc;

    use super::{IdentityResolver, run_application, run_application_with_timeout};

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

    struct HangingResolver;

    impl IdentityResolver for HangingResolver {
        fn resolve(&mut self, request: ResolutionRequest) -> ApplicationIdentity {
            thread::sleep(Duration::from_millis(200));
            ApplicationIdentity {
                pid: request.pid,
                app_id: None,
                display_name: String::from("too late"),
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

    #[tokio::test]
    async fn resolver_timeout_preserves_identity_and_pipeline_progress() {
        let (incoming_tx, incoming_rx) = mpsc::channel(4);
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(4);
        let task = tokio::spawn(run_application_with_timeout(
            incoming_rx,
            outgoing_tx,
            HangingResolver,
            MonitorState::new(),
            Duration::from_millis(20),
        ));
        let started = Instant::now();

        let first = session();
        let mut second = session();
        second.id = SessionId::new("session-two").unwrap();
        second.application.display_name = String::from("Second fallback");
        incoming_tx
            .send(MonitorEvent::SessionStarted(first))
            .await
            .unwrap();
        incoming_tx
            .send(MonitorEvent::SessionStarted(second))
            .await
            .unwrap();
        drop(incoming_tx);

        let first = outgoing_rx.recv().await.unwrap();
        let second = outgoing_rx.recv().await.unwrap();
        assert!(started.elapsed() < Duration::from_millis(100));
        let MonitorEvent::SessionStarted(first) = first else {
            panic!("expected first start");
        };
        let MonitorEvent::SessionStarted(second) = second else {
            panic!("expected second start");
        };
        assert_eq!(first.application.display_name, "Unknown application");
        assert_eq!(second.application.display_name, "Second fallback");

        assert_eq!(task.await.unwrap().unwrap().active_session_count(), 2);
    }
}
