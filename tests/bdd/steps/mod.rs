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

mod cli;
#[cfg(unix)]
mod fs;
mod ir;
mod manifest;
mod manifest_command;
mod ninja;
mod process;
mod stdlib;

// Step functions are registered via macros, so we don't need to re-export
// them explicitly. The macros generate global step registrations.
