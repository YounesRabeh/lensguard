//! Stable user-session D-Bus adapter for `LensGuard`.
//!
//! Transport DTOs are deliberately separate from `camera-core` domain entities. The public
//! contract exposes stable relationship and device identifiers, but no privileged observer data.

mod dto;
mod error;
mod service;

pub use dto::SessionDto;
pub use error::{CameraMonitorError, ServiceError};
pub use service::DbusService;

pub const BUS_NAME: &str = "io.github.younesrabeh.CameraMonitor";
pub const OBJECT_PATH: &str = "/io/github/younesrabeh/CameraMonitor";
pub const INTERFACE_NAME: &str = "io.github.younesrabeh.CameraMonitor1";
pub const PING_RESPONSE: &str = "pong";
pub const INTROSPECTION_XML: &str =
    include_str!("../../../dbus/io.github.younesrabeh.CameraMonitor1.xml");

/// The workspace version exposed over D-Bus.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
