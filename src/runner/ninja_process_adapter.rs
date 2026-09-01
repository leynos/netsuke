//! Translation between parsed CLI state and the Ninja process adapter.
//!
//! This module owns the one-way `Cli` to `NinjaProcessOptions` translation and
//! the public compatibility wrappers. Runner command handlers and callers with
//! a `Cli` use these wrappers; process requests remain parser-independent and
//! callers without CLI state construct `NinjaProcessOptions` directly.

use super::{BuildTargets, CommandEnv, StderrMode, process};
use crate::cli::Cli;
use camino::Utf8Path;
use std::io;

/// Translate CLI state into the narrow options consumed by the process layer.
///
/// # Errors
///
/// Returns [`io::ErrorKind::InvalidInput`] when the job count lies outside the
/// supported `1..=64` range.
pub(super) fn ninja_process_options(cli: &Cli) -> io::Result<process::NinjaProcessOptions> {
    let working_dir = cli.directory.clone();
    let jobs = cli.jobs.map(process::NinjaJobCount::try_new).transpose()?;
    Ok(process::NinjaProcessOptions { working_dir, jobs })
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
    program: &Utf8Path,
    cli: &Cli,
    build_file: &Utf8Path,
    targets: &BuildTargets<'_>,
) -> std::io::Result<()> {
    let options = ninja_process_options(cli)?;
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
    program: &Utf8Path,
    cli: &Cli,
    build_file: &Utf8Path,
    tool: &str,
) -> std::io::Result<()> {
    let options = ninja_process_options(cli)?;
    process::run_ninja_tool_with(&process::NinjaToolRequest {
        program,
        options: &options,
        build_file,
        tool,
        env: &CommandEnv::inherit(),
        stderr_mode: StderrMode::from_json_enabled(cli.json),
    })
}

#[cfg(test)]
mod tests {
    //! Unit tests for CLI-to-process option translation.

    use super::*;
    use anyhow::{Result, ensure};
    use std::io::ErrorKind;

    #[test]
    fn ninja_process_options_rejects_out_of_range_job_counts() -> Result<()> {
        // 0 and 65 sit just outside the supported 1..=64 bound; the CLI's own
        // boundary tests assert the same values.
        for jobs in [0, 65] {
            let cli = Cli {
                jobs: Some(jobs),
                ..Cli::default()
            };
            let Err(error) = ninja_process_options(&cli) else {
                anyhow::bail!("job count {jobs} should be rejected");
            };
            ensure!(
                error.kind() == ErrorKind::InvalidInput,
                "job count {jobs} returned {:?}, not InvalidInput",
                error.kind()
            );
        }
        Ok(())
    }

    #[test]
    fn ninja_process_options_preserves_a_valid_job_count() -> Result<()> {
        let cli = Cli {
            jobs: Some(8),
            ..Cli::default()
        };
        let options = ninja_process_options(&cli)?;
        let expected = process::NinjaJobCount::try_new(8)?;
        ensure!(
            options.jobs == Some(expected),
            "a present valid job count should be preserved, got {:?}",
            options.jobs
        );
        Ok(())
    }
}
