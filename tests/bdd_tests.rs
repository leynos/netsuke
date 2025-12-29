//! BDD test entry point using rstest-bdd.
//!
//! This file serves as the main test binary that discovers and runs all BDD
//! scenarios from the feature files. The scenarios! macro generates test
//! functions for each scenario found in the feature directories.

mod bdd;

// Re-export fixtures for scenario functions
pub use bdd::fixtures::*;

// Step definitions are registered via macros in the steps submodules.
// We only need to import the modules so the registration code runs.

use rstest_bdd_macros::scenarios;

// Autodiscover all cross-platform scenarios from feature files
// The fixtures parameter ensures TestWorld is injected into each generated test
scenarios!("tests/features", fixtures = [world: TestWorld]);

// Autodiscover Unix-only scenarios (gated by compile-time cfg)
#[cfg(unix)]
scenarios!("tests/features_unix", fixtures = [world: TestWorld]);
