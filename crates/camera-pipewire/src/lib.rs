//! `PipeWire` registry observation for `LensGuard`.
//!
//! This crate owns every raw `PipeWire` global identifier and translates registry callbacks into
//! a testable [`RegistryEvent`] stream. Step 3 classifies graph objects but deliberately does not
//! correlate links into domain camera sessions.

pub mod error;
pub mod graph;
mod mapper;
mod registry;

pub use error::{MappingError, PipeWireError};
pub use graph::{
    NodeClassification, PortClassification, PortDirection, PropertyMap, RawGraph, RawLink, RawNode,
    RawPort, RegistryEvent, RegistryObjectKind,
};
pub use registry::inspect_pipewire;

/// The workspace version used by this crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
