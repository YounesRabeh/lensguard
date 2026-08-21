//! Infrastructure-independent domain types and state management for `LensGuard`.
//!
//! This crate deliberately contains no desktop, asynchronous-runtime, or transport-specific
//! dependencies. Adapters exchange [`MonitorEvent`] values with the domain and consume ordered
//! [`MonitorSnapshot`] values through the traits in [`ports`].

pub mod error;
pub mod event;
pub mod model;
pub mod ports;
pub mod state;
pub mod v4l2;

pub use error::{DomainError, IdentifierKind};
pub use event::MonitorEvent;
pub use model::{
    ApplicationIdentity, CameraDevice, CameraSession, CameraSessionState, DeviceId, SessionId,
};
pub use ports::{CameraEventSource, SessionObserver};
pub use state::{MonitorSnapshot, MonitorState};
pub use v4l2::{
    CaptureConfidence, CaptureOwner, CaptureTransition, DirectCaptureEvent, DirectCaptureOperation,
    EventValidationError, MAX_OBSERVER_MESSAGE_SIZE, OBSERVER_SCHEMA_VERSION, ObserverAvailability,
    ObserverClientMessage, ObserverMessage, PhysicalCameraId, SuppressionDiagnostics,
    V4l2SessionMachine,
};

/// The workspace version used by this crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
