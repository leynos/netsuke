//! Turn a typed Ninja invocation failure into the diagnostic the runner reports.
//!
//! Kept beside the runner dispatch rather than inside it so the mapping from
//! process failure to user-facing diagnostic has one home, and so the dispatch
//! module stays within the repository's module-size limit.

use super::{RunnerError, process};
use crate::localization::{self, keys};
use crate::terminal_text::terminal_safe;
use std::process::ExitStatus;

/// Render an exit status in the active locale.
///
/// `ExitStatus`'s own `Display` is English-only and platform-defined, so
/// embedding it would leave an English fragment, worded differently on Unix
/// and Windows, inside an otherwise translated summary. The command layer
/// already words both an exit code and a signal in every catalogue, so the
/// summary reuses those suffixes and keeps the raw status in the cause chain
/// for callers that need it.
fn localized_status(status: ExitStatus) -> String {
    status.code().map_or_else(
        || localization::message(keys::COMMAND_SIGNAL_SUFFIX).to_string(),
        |code| {
            localization::message(keys::COMMAND_EXIT_STATUS_SUFFIX)
                .with_arg("status", code)
                .to_string()
        },
    )
}

/// Convert a typed Ninja failure into the diagnostic the runner reports.
///
/// A launch or exit failure describes itself, so it must not inherit the
/// invocation context: that context names the temporary build file, which says
/// nothing about what went wrong. Every other process failure keeps the
/// context, because the invocation details are where an operator has to look.
pub(super) fn ninja_failure(
    failure: process::NinjaInvocationError,
    context: impl FnOnce() -> String,
) -> anyhow::Error {
    match failure {
        process::NinjaInvocationError::Spawn { program, source } => {
            let escaped = terminal_safe(program.as_str()).into_owned();
            RunnerError::NinjaSpawnFailed {
                program: program.into_string(),
                message: localization::message(keys::RUNNER_NINJA_SPAWN_FAILED)
                    .with_arg("program", escaped.clone()),
                help: localization::message(keys::RUNNER_NINJA_SPAWN_FAILED_HELP)
                    .with_arg("program", escaped),
                source,
            }
            .into()
        }
        process::NinjaInvocationError::Exit(exit) => {
            let status = exit.status();
            let status_text = localized_status(status);
            let message = if exit.operation() == "build" {
                localization::message(keys::RUNNER_NINJA_BUILD_FAILED)
                    .with_arg("status", status_text)
            } else {
                localization::message(keys::RUNNER_NINJA_TOOL_FAILED)
                    .with_arg("tool", terminal_safe(exit.operation()))
                    .with_arg("status", status_text)
            };
            RunnerError::NinjaFailed {
                status,
                operation: exit.operation().to_owned(),
                message,
                source: exit,
            }
            .into()
        }
        process::NinjaInvocationError::Io(source) => anyhow::Error::new(source).context(context()),
    }
}
