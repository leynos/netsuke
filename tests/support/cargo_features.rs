//! Select the Cargo packages and targets needed by direct-rustc UI harnesses.
//!
//! Each harness checks only the library whose artefacts it consumes and passes
//! its package name at the call site. Checking the `netsuke-build` library also
//! runs its build script and prepares its host-side build dependencies. The
//! `test_support` library check includes its dependency on `netsuke-build`.
//! Both checks enable all features without compiling unrelated test targets.
//! Isolated fixture and packaging builds retain shipped defaults and must not
//! use this selection.

/// Select library targets with all package features for direct-rustc UI fixtures.
pub const LIBRARY_BUILD_ARGUMENTS: &[&str] = &["--lib", "--all-features"];
