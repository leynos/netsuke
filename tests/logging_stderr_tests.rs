//! Integration tests for stderr logging and JSON output contracts.

#[cfg(unix)]
#[path = "logging_stderr/command_list_failure.rs"]
mod command_list_failure;
#[cfg(unix)]
#[path = "support/command_list_failure.rs"]
mod command_list_failure_support;
#[path = "logging_stderr/config_tracing.rs"]
mod config_tracing;
#[path = "logging_stderr/json.rs"]
mod json;
#[path = "logging_stderr/support.rs"]
mod support;
#[path = "logging_stderr/verbose.rs"]
mod verbose;
#[path = "logging_stderr/verbose_secret_absence.rs"]
mod verbose_secret_absence;
