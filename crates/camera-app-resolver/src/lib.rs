//! Application identity enrichment for `LensGuard`.
//!
//! The resolver combines trusted backend hints with permitted process metadata and desktop-entry
//! records. It never reads process command lines, returns icon paths, or makes missing process
//! information fatal to a camera session.

mod desktop_entry;
mod error;
mod flatpak;
mod procfs;
mod resolver;

pub use desktop_entry::{DesktopEntry, DesktopEntryIndex};
pub use error::{DesktopEntryError, ProcfsError, ResolverError};
pub use procfs::{ProcessSnapshot, ProcessSource, ProcfsReader};
pub use resolver::{ApplicationResolver, ResolutionRequest};

/// The workspace version used by this crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
