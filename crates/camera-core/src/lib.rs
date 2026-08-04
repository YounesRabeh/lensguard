//! Infrastructure-independent domain boundary for `LensGuard`.
//!
//! Domain models and behavior are intentionally deferred to Step 2.

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
