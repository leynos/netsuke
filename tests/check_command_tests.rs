//! End-to-end contracts for the `netsuke check` command.
//!
//! These run the built binary, because the properties under test are the ones
//! only the process boundary can show: the exit code, which stream each
//! document reaches, and that stdout stays empty when a run fails in JSON
//! mode. The child modules group output, policy, and explanation contracts.

#[path = "check_command_tests/support.rs"]
mod support;

#[path = "check_command_tests/output.rs"]
mod output;

#[path = "check_command_tests/policy.rs"]
mod policy;

#[path = "check_command_tests/explanation.rs"]
mod explanation;
