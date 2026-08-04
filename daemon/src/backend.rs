use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use camera_core::MonitorEvent;
use camera_pipewire::{PipeWireError, PipeWireEventSource};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::backoff::BackoffPolicy;

const SOURCE_POLL_INTERVAL: Duration = Duration::from_millis(100);
const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Pollable event source used by the blocking backend supervisor.
pub trait BackendSource: Send {
    type Error: Error + Send + Sync + 'static;

    /// Waits up to `timeout` for one event. `None` means that the timeout elapsed.
    ///
    /// # Errors
    ///
    /// Returns the backend-specific observation failure.
    fn next_event_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<MonitorEvent>, Self::Error>;
}

impl BackendSource for PipeWireEventSource {
    type Error = PipeWireError;

    fn next_event_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<MonitorEvent>, Self::Error> {
        PipeWireEventSource::next_event_timeout(self, timeout)
    }
}

/// Factory boundary that permits retryable real and fake backend connections.
pub trait BackendFactory: Send + Sync + 'static {
    type Source: BackendSource;
    type Error: Error + Send + Sync + 'static;

    /// Establishes a fresh backend observation source.
    ///
    /// # Errors
    ///
    /// Returns the backend-specific connection or initialization failure.
    fn connect(&self) -> Result<Self::Source, Self::Error>;
}

/// Production `PipeWire` backend factory.
#[derive(Clone, Copy, Debug, Default)]
pub struct PipeWireBackendFactory;

impl BackendFactory for PipeWireBackendFactory {
    type Source = PipeWireEventSource;
    type Error = PipeWireError;

    fn connect(&self) -> Result<Self::Source, Self::Error> {
        PipeWireEventSource::connect()
    }
}

/// Terminal reason for the blocking backend supervisor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisorExit {
    Shutdown,
    ConsumerClosed,
}

/// Connects, polls, and reconnects a backend until cancellation or downstream shutdown.
///
/// This function is blocking and must run on a dedicated thread or `spawn_blocking` worker.
pub fn run_backend_supervisor<F>(
    factory: &F,
    events: &mpsc::Sender<MonitorEvent>,
    shutdown: &Arc<AtomicBool>,
    backoff: BackoffPolicy,
) -> SupervisorExit
where
    F: BackendFactory,
{
    let mut attempt = 0_u32;
    loop {
        if shutdown.load(Ordering::Acquire) {
            return SupervisorExit::Shutdown;
        }

        let mut source = match factory.connect() {
            Ok(source) => {
                info!("PipeWire backend connected");
                attempt = 0;
                if !send(events, shutdown, MonitorEvent::BackendRecovered) {
                    return send_failure_exit(shutdown);
                }
                source
            }
            Err(error) => {
                let reason = format!("PipeWire backend connection failed: {error}");
                warn!(attempt, %error, "PipeWire backend unavailable; retrying");
                if !send(
                    events,
                    shutdown,
                    MonitorEvent::BackendUnavailable { reason },
                ) {
                    return send_failure_exit(shutdown);
                }
                let delay = backoff.delay(attempt);
                attempt = attempt.saturating_add(1);
                if wait_for_shutdown(shutdown, delay) {
                    return SupervisorExit::Shutdown;
                }
                continue;
            }
        };

        loop {
            if shutdown.load(Ordering::Acquire) {
                return SupervisorExit::Shutdown;
            }
            match source.next_event_timeout(SOURCE_POLL_INTERVAL) {
                Ok(Some(event)) => {
                    if !send(events, shutdown, event) {
                        return send_failure_exit(shutdown);
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    let reason = format!("PipeWire backend disconnected: {error}");
                    warn!(%error, "PipeWire backend disconnected; reconciling and retrying");
                    if !send(
                        events,
                        shutdown,
                        MonitorEvent::BackendUnavailable { reason },
                    ) {
                        return send_failure_exit(shutdown);
                    }
                    let delay = backoff.delay(attempt);
                    attempt = attempt.saturating_add(1);
                    if wait_for_shutdown(shutdown, delay) {
                        return SupervisorExit::Shutdown;
                    }
                    break;
                }
            }
        }
    }
}

fn send_failure_exit(shutdown: &AtomicBool) -> SupervisorExit {
    if shutdown.load(Ordering::Acquire) {
        SupervisorExit::Shutdown
    } else {
        SupervisorExit::ConsumerClosed
    }
}

fn send(events: &mpsc::Sender<MonitorEvent>, shutdown: &AtomicBool, event: MonitorEvent) -> bool {
    if shutdown.load(Ordering::Acquire) {
        return false;
    }
    if events.blocking_send(event).is_err() {
        debug!("application event consumer closed");
        return false;
    }
    true
}

fn wait_for_shutdown(shutdown: &AtomicBool, duration: Duration) -> bool {
    let deadline = Instant::now() + duration;
    while !shutdown.load(Ordering::Acquire) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return false;
        }
        thread::sleep(remaining.min(SHUTDOWN_POLL_INTERVAL));
    }
    true
}
