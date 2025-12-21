//! BDD test module providing fixtures and step definitions.
//!
//! Step definitions are registered via `#[given]`, `#[when]`, and `#[then]`
//! attribute macros from rstest-bdd.

pub mod fixtures;
pub mod steps;
pub mod types;
