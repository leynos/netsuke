//! Translation between parsed CLI state and the Ninja process adapter.
//!
//! This module owns the one-way `Cli` to `NinjaProcessOptions` translation and
//! the public compatibility wrappers. Runner command handlers and callers with
//! a `Cli` use these wrappers; process requests remain parser-independent and
//! callers without CLI state construct `NinjaProcessOptions` directly.

use super::{BuildTargets, CommandEnv, StderrMode, process};
use crate::cli::Cli;
use std::path::Path;

/// Translate CLI state into the narrow options consumed by the process layer.
pub(super) fn ninja_process_options(cli: &Cli) -> process::NinjaProcessOptions {
    process::NinjaProcessOptions {
        working_dir: cli.directory.clone(),
        jobs: cli.jobs,
    }
}

/// Invoke the Ninja executable with the provided CLI settings.
///
/// This compatibility wrapper translates parser state at the orchestration
/// boundary before delegating to the process adapter.
///
/// # Errors
///
/// Returns an [`std::io::Error`] if Ninja cannot execute successfully.
pub fn run_ninja(
    program: &Path,
    cli: &Cli,
    build_file: &Path,
    targets: &BuildTargets<'_>,
) -> std::io::Result<()> {
    let options = ninja_process_options(cli);
    process::run_ninja_with(&process::NinjaBuildRequest {
        program,
        options: &options,
        build_file,
        targets,
        env: &CommandEnv::inherit(),
        stderr_mode: StderrMode::from_json_enabled(cli.json),
    })
}

/// Invoke a Ninja tool with the provided CLI settings.
///
/// This compatibility wrapper translates parser state at the orchestration
/// boundary before delegating to the process adapter.
///
/// # Errors
///
/// Returns an [`std::io::Error`] if Ninja cannot execute successfully.
pub fn run_ninja_tool(
    program: &Path,
    cli: &Cli,
    build_file: &Path,
    tool: &str,
) -> std::io::Result<()> {
    let options = ninja_process_options(cli);
    process::run_ninja_tool_with(&process::NinjaToolRequest {
        program,
        options: &options,
        build_file,
        tool,
        env: &CommandEnv::inherit(),
        stderr_mode: StderrMode::from_json_enabled(cli.json),
    })
}
