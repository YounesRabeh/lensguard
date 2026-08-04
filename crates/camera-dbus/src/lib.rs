//! D-Bus adapter boundary for `LensGuard`.
//!
//! The public D-Bus contract is intentionally deferred to Step 6.

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
