//! CLI integration test harness.
//!
//! This file activates the `tests/cli_tests/` module tree as a Cargo
//! integration test target so parsing, merge, locale, and policy tests run
//! under `cargo test` and `make test`.

#[path = "cli_tests/mod.rs"]
mod cli_tests;
