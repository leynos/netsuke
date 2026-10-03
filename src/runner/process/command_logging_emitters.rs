//! Bounded tracing emitters for prepared Ninja subprocess commands.
//!
//! Each emitter owns exactly one `tracing` macro so the caller's cognitive
//! complexity stays structural: the `log` feature, enabled transitively by a
//! dev-dependency, expands every tracing macro into extra branches. The
//! measurement and its rationale live in the migration ExecPlan; the rule for
//! new code is that a `tracing` macro invoked from a function under the
//! complexity threshold belongs in an emitter like these.

use super::super::StderrMode;
use super::CommandLogContext;
use tracing::{debug, info};

/// Emit the bounded execution summary for one Ninja subprocess.
pub(super) fn info_command_execution_from_fields(
    context: &CommandLogContext,
    operation: &str,
    stderr_mode: StderrMode,
) {
    info!(
        operation,
        ninja_program = %context.program_display,
        arg_count = context.arg_count,
        env_override_count = context.env_override_count,
        path_overridden = context.is_path_overridden,
        suppress_stderr = stderr_mode.is_suppress(),
        "Executing Ninja subprocess",
    );
}

/// Emit the redacted command line for one Ninja subprocess.
pub(super) fn debug_command_line_from_fields(
    context: &CommandLogContext,
    operation: &str,
    stderr_mode: StderrMode,
) {
    debug!(
        operation,
        ninja_program = %context.program_display,
        suppress_stderr = stderr_mode.is_suppress(),
        "Executing command: {}",
        context.redacted_command,
    );
}
