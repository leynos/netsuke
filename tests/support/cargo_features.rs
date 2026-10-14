//! Select the Cargo packages and targets needed by direct-rustc UI harnesses.
//!
//! Each harness checks only the library whose artefacts it consumes and passes
//! its package name at the call site. Checking the `netsuke-build` library also
//! runs its build script and prepares its host-side build dependencies. The
//! `test_support` library check includes its dependency on `netsuke-build`.
//! Both checks use the feature selection of the lane that compiled the test,
//! without compiling unrelated test targets.
//! Isolated fixture and packaging builds retain shipped defaults and must not
//! use this selection.

/// Select library targets with every package feature for direct-rustc UI fixtures.
///
/// The gate runs twice: `make test` with `--all-features`, and
/// `make test-default-features` with none, which is the set release binaries
/// build. A nested check must match whichever lane compiled this test, or Cargo
/// fingerprints `netsuke-build` differently and recompiles its whole graph
/// inside the test. `lint` is on only in the all-features lane, so it
/// identifies the lane.
#[cfg(feature = "lint")]
pub const LIBRARY_BUILD_ARGUMENTS: &[&str] = &["--lib", "--all-features"];

/// Select library targets with shipped default features for direct-rustc UI
/// fixtures in the `make test-default-features` lane.
#[cfg(not(feature = "lint"))]
pub const LIBRARY_BUILD_ARGUMENTS: &[&str] = &["--lib"];
