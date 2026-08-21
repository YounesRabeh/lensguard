use crate::{CameraSession, ObserverAvailability, SessionId, SuppressionDiagnostics};

/// A backend observation applied to [`MonitorState`](crate::MonitorState).
///
/// Events carry domain values only. Adapter-specific handles and object identifiers must be
/// translated before an event crosses this boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MonitorEvent {
    /// A new active relationship was observed.
    SessionStarted(CameraSession),
    /// Metadata for an existing relationship became more complete.
    SessionUpdated(CameraSession),
    /// An active relationship ended.
    SessionStopped(SessionId),
    /// Availability of the privileged V4L2 observer changed.
    ObserverAvailabilityChanged {
        availability: ObserverAvailability,
        detail: String,
    },
    /// Non-identifying suppression totals changed.
    SuppressionDiagnosticsChanged(SuppressionDiagnostics),
}
