//! Shares the Cargo target and feature selection used by ambient UI checks.
//!
//! Scope: direct-rustc UI harnesses use this module when they need Cargo to
//! check workspace packages with the test gate's workspace, target, and feature
//! selection. Test targets unify development-dependency features that a
//! library-only check would omit. Cargo check supplies metadata artefacts
//! without linking integration executables; its build mode and cache identity
//! need not match the test gate. Isolated fixture and packaging builds retain
//! shipped defaults and must not use this selection.

/// Select the workspace, targets, and features used by the test gate.
pub const GATE_BUILD_ARGUMENTS: &[&str] = &["--workspace", "--all-targets", "--all-features"];
