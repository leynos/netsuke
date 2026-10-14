//! Shares the Cargo target and feature selection used by ambient UI builds.
//!
//! Scope: direct-rustc UI harnesses that build workspace packages in the
//! ambient target directory use this module so their dependency fingerprints
//! match the test gate. Target selection matters because test dependencies
//! enable features absent from library-only builds. Isolated fixture and
//! packaging builds retain their shipped defaults and must not use this
//! selection.
//! Callers use `cargo test` with `--no-run` to preserve the test build mode
//! without recursively executing the suite inside a fixture.

/// Pass the workspace, target, and feature selection used by the test gate.
pub const GATE_BUILD_ARGUMENTS: &[&str] =
    &["--no-run", "--workspace", "--all-targets", "--all-features"];
