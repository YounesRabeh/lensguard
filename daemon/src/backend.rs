use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use camera_core::MonitorEvent;
use camera_core::ObserverAvailability;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::backoff::BackoffPolicy;
use crate::observer::{ObserverError, ObserverEventSource};

const SOURCE_POLL_INTERVAL: Duration = Duration::from_millis(100);
const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(50);
const MAX_FAILURE_TEXT_CHARS: usize = 512;

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

impl BackendSource for ObserverEventSource {
    type Error = ObserverError;

    fn next_event_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<MonitorEvent>, Self::Error> {
        ObserverEventSource::next_event_timeout(self, timeout)
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

    #[must_use]
    fn connected_event() -> Option<MonitorEvent> {
        Some(MonitorEvent::ObserverAvailabilityChanged {
            availability: ObserverAvailability::Available,
            detail: String::new(),
        })
    }

    fn unavailable_event(error: &Self::Error) -> MonitorEvent {
        MonitorEvent::ObserverAvailabilityChanged {
            availability: ObserverAvailability::ConnectionFailed,
            detail: bounded_failure_text(&error.to_string()),
        }
    }
}

/// Production direct-V4L2 observer factory.
#[derive(Clone, Copy, Debug, Default)]
pub struct ObserverBackendFactory;

impl BackendFactory for ObserverBackendFactory {
    type Source = ObserverEventSource;
    type Error = ObserverError;

    fn connect(&self) -> Result<Self::Source, Self::Error> {
        ObserverEventSource::connect()
    }

    fn connected_event() -> Option<MonitorEvent> {
        None
    }

    fn unavailable_event(error: &Self::Error) -> MonitorEvent {
        MonitorEvent::ObserverAvailabilityChanged {
            availability: error.availability(),
            detail: bounded_failure_text(&error.to_string()),
        }
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
                info!("V4L2 observer connected");
                attempt = 0;
                if let Some(event) = F::connected_event()
                    && !send(events, shutdown, event)
                {
                    return send_failure_exit(shutdown);
                }
                source
            }
            Err(error) => {
                let unavailable = F::unavailable_event(&error);
                let error = bounded_failure_text(&error.to_string());
                warn!(attempt, %error, "V4L2 observer unavailable; retrying");
                if !send(events, shutdown, unavailable) {
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
                    let error = bounded_failure_text(&error.to_string());
                    let reason = format!("V4L2 observer disconnected: {error}");
                    warn!(%error, "V4L2 observer disconnected; reconciling and retrying");
                    if !send(
                        events,
                        shutdown,
                        MonitorEvent::ObserverAvailabilityChanged {
                            availability: ObserverAvailability::BackendLost,
                            detail: reason,
                        },
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

fn bounded_failure_text(value: &str) -> String {
    value.chars().take(MAX_FAILURE_TEXT_CHARS).collect()
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

#[cfg(test)]
mod tests {
    use super::{MAX_FAILURE_TEXT_CHARS, bounded_failure_text};

    #[test]
    fn externally_supplied_failure_text_is_bounded_without_invalid_utf8() {
        let bounded = bounded_failure_text(&"📷".repeat(10_000));
        assert_eq!(bounded.chars().count(), MAX_FAILURE_TEXT_CHARS);
    }
}
