//! Shares the Cargo feature selection used by ambient UI harness builds.
//!
//! Scope: direct-rustc UI harnesses that build workspace packages in the
//! ambient target directory use this module so their Cargo fingerprints match
//! the test gate. Isolated fixture and packaging builds retain their shipped
//! default features and must not use this selection.

/// Pass the same feature selection used by `make test-nextest`.
///
/// The gate runs twice: `make test` with `--all-features`, and
/// `make test-default-features` with none, which is the set release binaries
/// build. A nested build must match whichever lane compiled this test, or Cargo
/// fingerprints `netsuke-build` differently and recompiles its whole graph
/// inside the test. `lint` is on only in the all-features lane, so it
/// identifies the lane.
#[cfg(feature = "lint")]
pub const GATE_FEATURE_ARGUMENTS: &[&str] = &["--all-features"];

/// Pass no feature selection, matching `make test-default-features`.
#[cfg(not(feature = "lint"))]
pub const GATE_FEATURE_ARGUMENTS: &[&str] = &[];
