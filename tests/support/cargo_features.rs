//! Shares the Cargo feature selection used by ambient UI harness builds.
//!
//! Scope: direct-rustc UI harnesses that build workspace packages in the
//! ambient target directory use this module so their Cargo fingerprints match
//! the test gate. Isolated fixture and packaging builds retain their shipped
//! default features and must not use this selection.

/// Pass the same feature selection used by `make test-nextest`.
pub const GATE_FEATURE_ARGUMENTS: &[&str] = &["--all-features"];
