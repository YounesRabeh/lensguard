//! Application orchestration for the `LensGuard` user daemon.

pub mod application;
pub mod backend;
pub mod backoff;
pub mod config;
pub mod runtime;

pub use config::{Command, Config, ConfigError, USAGE};
pub use runtime::{APPLICATION_QUEUE_CAPACITY, RuntimeError, run_daemon, run_standalone_dbus};
