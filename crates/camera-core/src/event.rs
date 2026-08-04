use crate::{CameraSession, SessionId};

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
    /// The monitoring backend cannot currently observe session state.
    BackendUnavailable { reason: String },
    /// The monitoring backend resumed observation.
    BackendRecovered,
}
