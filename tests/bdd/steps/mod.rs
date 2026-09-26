//! Step definition modules for BDD scenarios.
//!
//! Each module contains step definitions for a specific domain. Steps are
//! registered via `#[given]`, `#[when]`, and `#[then]` attribute macros.
//!
//! ## Lint suppressions
//!
//! The `rstest-bdd` macros generate wrapper code for each step function that
//! triggers multiple Clippy lints. Since the generated code cannot be
//! annotated directly, function-level `#[expect(...)]` attributes are applied
//! to each step that triggers a lint.
//!
//! FIXME(rstest-bdd): Once <https://github.com/leynos/rstest-bdd/issues/381>
//! is resolved, these suppressions may be removable.
//!
//! Migrating to `rstest-bdd` 0.6.0 did not change any of this. All 45
//! suppressions remain achieved, verified by the `-D warnings` build inside
//! `make test-nextest` — not by `make lint`, which aborts in `src/` first.
//! Remove one only when the compiler proves its lint no longer fires.

mod accessibility_preferences;
mod accessible_output;
mod advanced_usage;
mod cli;
mod cli_config;
mod cli_parsing;
mod conditional_manifest;
mod configuration_discovery;
mod configuration_preferences;
mod documentation_examples;
#[cfg(unix)]
mod fs;
mod help_targets;
mod ir;
mod json_diagnostics;
mod locale_resolution;
mod manifest;
mod manifest_command;
mod ninja;
mod process;
mod progress_output;
mod stdlib;

// Step functions are registered via macros, so we don't need to re-export
// them explicitly. The macros generate global step registrations.
