//! Step definition modules for BDD scenarios.
//!
//! Each module contains step definitions for a specific domain. Steps are
//! registered via `#[given]`, `#[when]`, and `#[then]` attribute macros.
//!
//! ## File-wide lint suppressions
//!
//! The `rstest-bdd` macros generate wrapper code for each step function that
//! triggers multiple Clippy lints. Since the generated code cannot be
//! annotated directly and these patterns appear across dozens of step
//! functions, file-wide suppressions are used here as a practical exception
//! to the project's "no blanket suppressions" guideline.
//!
//! FIXME(rstest-bdd): Once <https://github.com/nickkuk/rstest-bdd/issues/TBD>
//! is resolved, these suppressions may be removable.
//!
//! Suppressed lints and rationale:
//! - `shadow_reuse`: Step functions strip quotes via intentional shadowing
//! - `unnecessary_wraps`: Macros require `Result` returns for all steps
//! - `str_to_string`: Quote stripping converts `&str` parameters
//! - `redundant_closure_for_method_calls`: Closures improve step readability
//! - `needless_pass_by_value`: Step signatures prioritise ergonomics
//! - `option_if_let_else`: if-let-else is clearer for step logic
//! - `doc_markdown`: Informal docs don't need backticks on identifiers
//! - `redundant_closure`: Closures aid comprehension in step context

#![expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macros generate step wrappers that shadow parameters"
)]
#![expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macros require Result returns for step functions"
)]
#![expect(
    clippy::str_to_string,
    reason = "rstest-bdd step functions convert quote-stripped parameters"
)]
#![expect(
    clippy::redundant_closure_for_method_calls,
    reason = "rstest-bdd step closures prioritise readability over brevity"
)]
#![expect(
    clippy::needless_pass_by_value,
    reason = "rstest-bdd step signatures prioritise ergonomics"
)]
#![expect(
    clippy::option_if_let_else,
    reason = "rstest-bdd step logic uses if-let-else for clarity"
)]
#![expect(
    clippy::doc_markdown,
    reason = "rstest-bdd step docs are informal and omit backticks"
)]
#![expect(
    clippy::redundant_closure,
    reason = "rstest-bdd step closures improve comprehension"
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
