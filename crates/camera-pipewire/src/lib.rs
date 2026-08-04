//! `PipeWire` graph observation and camera-session correlation for `LensGuard`.
//!
//! This crate owns every raw `PipeWire` global identifier and translates registry callbacks into
//! a testable [`RegistryEvent`] stream. Correlation produces backend-independent domain events
//! only when a complete camera-to-application relationship exists.

mod correlation;
pub mod error;
pub mod graph;
mod mapper;
mod observer;
mod registry;
mod source;

pub use correlation::CorrelationEngine;
pub use error::{CorrelationError, MappingError, PipeWireError};
pub use graph::{
    NodeClassification, PortClassification, PortDirection, PropertyMap, RawGraph, RawLink, RawNode,
    RawPort, RegistryEvent, RegistryObjectKind,
};
pub use registry::inspect_pipewire;
pub use source::PipeWireEventSource;

/// The workspace version used by this crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
