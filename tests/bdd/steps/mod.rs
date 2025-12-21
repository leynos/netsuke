//! Step definition modules for BDD scenarios.
//!
//! Each module contains step definitions for a specific domain. Steps are
//! registered via `#[given]`, `#[when]`, and `#[then]` attribute macros.

// Step functions use shadow_reuse to strip quotes from captured parameters,
// which is idiomatic for this pattern. The rstest-bdd macros also generate
// code with these patterns.
#![allow(
    clippy::shadow_reuse,
    reason = "Step functions strip quotes using intentional shadowing"
)]
// The rstest-bdd macros generate functions that may have unnecessary wraps
#![allow(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macros require Result returns for step functions"
)]
// The step functions use to_string on &str for convenience
#![allow(
    clippy::str_to_string,
    reason = "Step functions convert stripped quotes"
)]
// Step functions may use closures for clarity even when methods exist
#![allow(
    clippy::redundant_closure_for_method_calls,
    reason = "Step closures are clearer than method references"
)]
// Some step functions take owned values for ergonomics
#![allow(
    clippy::needless_pass_by_value,
    reason = "Step function signatures optimized for readability"
)]
// Step logic may be clearer with if-let-else than map_or_else
#![allow(
    clippy::option_if_let_else,
    reason = "if-let-else is clearer for step logic"
)]
// Step comments may reference identifiers without backticks
#![allow(clippy::doc_markdown, reason = "Step docs are informal")]
// Step assertions may use owned values for comparison simplicity
#![allow(clippy::cmp_owned, reason = "Simpler comparisons in step assertions")]
// Step closures may be clearer than function references
#![allow(
    clippy::redundant_closure,
    reason = "Closures are clearer in step context"
)]

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
