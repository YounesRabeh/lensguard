use std::error::Error;

use crate::{MonitorEvent, MonitorSnapshot};

/// Port implemented by adapters that supply camera-monitoring events.
///
/// This interface is intentionally synchronous. An asynchronous adapter can bridge its runtime
/// or channel at the application boundary without adding an async-runtime dependency here.
pub trait CameraEventSource {
    /// Infrastructure-specific failure converted to a typed adapter error.
    type Error: Error + Send + Sync + 'static;

    /// Returns the next event, or `None` when the source has ended cleanly.
    ///
    /// # Errors
    ///
    /// Returns the adapter's typed error when it cannot obtain the next event.
    fn next_event(&mut self) -> Result<Option<MonitorEvent>, Self::Error>;
}

/// Port implemented by application-layer consumers of complete state snapshots.
pub trait SessionObserver {
    /// Infrastructure-specific failure converted to a typed observer error.
    type Error: Error + Send + Sync + 'static;

    /// Observes one deterministic state snapshot.
    ///
    /// # Errors
    ///
    /// Returns the observer's typed error when it cannot consume the snapshot.
    fn state_changed(&mut self, snapshot: &MonitorSnapshot) -> Result<(), Self::Error>;
}
