//! `PipeWire` adapter boundary for `LensGuard`.
//!
//! `PipeWire` integration is intentionally deferred to Step 3.

/// The workspace version used by this crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn exposes_workspace_version() {
        assert!(!VERSION.is_empty());
    }
}
