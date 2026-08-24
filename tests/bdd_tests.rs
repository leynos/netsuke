//! BDD test entry point using rstest-bdd.
//!
//! This file is the main test binary that discovers and runs all BDD scenarios
//! from the feature files. The `scenarios!` macro generates a test function
//! for each scenario found in the feature directories.

mod bdd;
pub mod documentation_examples;

// Re-export fixtures for scenario functions
pub use bdd::fixtures::*;

// Step definitions are registered via macros in the steps submodules.
// We only need to import the modules so the registration code runs.

use rstest_bdd_macros::scenarios;

// Autodiscover all cross-platform scenarios from the canonical CLI feature files.
// The fixtures parameter ensures TestWorld is injected into each generated test
scenarios!("tests/features", fixtures = [world: TestWorld]);

// Autodiscover Unix-only scenarios (gated by compile-time cfg)
#[cfg(unix)]
scenarios!("tests/features_unix", fixtures = [world: TestWorld]);
