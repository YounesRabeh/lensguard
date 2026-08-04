use thiserror::Error;

use camera_core::DomainError;

/// A malformed property or fixture at the raw adapter boundary.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum MappingError {
    #[error("property '{key}' has invalid unsigned integer value '{value}'")]
    InvalidUnsignedInteger { key: String, value: String },
    #[error("fixture line {line} is invalid: {details}")]
    InvalidFixture { line: usize, details: String },
    #[error("fixture object ending at line {line} is missing {field}")]
    MissingFixtureField { line: usize, field: &'static str },
    #[error("fixture line {line} has unsupported object type '{value}'")]
    UnsupportedFixtureObjectType { line: usize, value: String },
}

/// Failures while converting a raw graph change into domain events.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CorrelationError {
    #[error("invalid PipeWire graph metadata: {0}")]
    Mapping(#[from] MappingError),
    #[error("could not create an opaque domain identifier: {0}")]
    Domain(#[from] DomainError),
}

/// Failures while observing the current user's `PipeWire` instance.
#[derive(Debug, Error)]
pub enum PipeWireError {
    #[error("failed to create the PipeWire main loop: {0}")]
    MainLoop(#[source] pipewire::Error),
    #[error("failed to create a PipeWire client context: {0}")]
    Context(#[source] pipewire::Error),
    #[error(
        "failed to connect to the user PipeWire instance: {0}; verify PipeWire is running and the session socket is accessible"
    )]
    Connect(#[source] pipewire::Error),
    #[error("failed to obtain the PipeWire registry: {0}")]
    Registry(#[source] pipewire::Error),
    #[error("failed to synchronize with the PipeWire registry: {0}")]
    Synchronize(#[source] pipewire::Error),
    #[error("PipeWire core error for object {object_id} (result {result}): {message}")]
    Core {
        object_id: u32,
        result: i32,
        message: String,
    },
    #[error("failed to start the PipeWire monitor thread: {0}")]
    MonitorThread(#[source] std::io::Error),
    #[error("failed to initialize the PipeWire monitor: {details}")]
    MonitorInitialization { details: String },
    #[error("PipeWire monitor initialization ended unexpectedly")]
    InitializationChannelClosed,
    #[error("PipeWire monitor event channel closed unexpectedly")]
    EventChannelClosed,
}
