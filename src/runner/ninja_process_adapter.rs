//! Translation between parsed CLI state and the Ninja process adapter.
//!
//! This module owns the one-way `Cli` to `NinjaProcessOptions` translation and
//! the public compatibility wrappers. Runner command handlers and callers with
//! a `Cli` use these wrappers; process requests remain parser-independent and
//! callers without CLI state construct `NinjaProcessOptions` directly.

use super::{BuildTargets, CommandEnv, StderrMode, process};
use crate::cli::Cli;
use camino::Utf8PathBuf;
use std::{
    io::{self, ErrorKind},
    path::Path,
};

/// Translate CLI state into the narrow options consumed by the process layer.
///
/// # Errors
///
/// Returns [`io::ErrorKind::InvalidData`] when the CLI working directory is
/// not valid UTF-8.
pub(super) fn ninja_process_options(cli: &Cli) -> io::Result<process::NinjaProcessOptions> {
    let working_dir = cli
        .directory
        .clone()
        .map(Utf8PathBuf::from_path_buf)
        .transpose()
        .map_err(|path| {
            io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "Ninja working directory {} is not valid UTF-8",
                    path.display()
                ),
            )
        })?;
    Ok(process::NinjaProcessOptions {
        working_dir,
        jobs: cli.jobs,
    })
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
    program: &Path,
    cli: &Cli,
    build_file: &Path,
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

    #[cfg(unix)]
    use std::os::unix::ffi::OsStringExt;

    #[cfg(unix)]
    #[test]
    fn ninja_process_options_rejects_non_utf8_working_directory() -> Result<()> {
        let cli = Cli {
            directory: Some(std::path::PathBuf::from(std::ffi::OsString::from_vec(
                vec![0xff],
            ))),
            ..Cli::default()
        };

        let Err(error) = ninja_process_options(&cli) else {
            anyhow::bail!("non-UTF-8 working directory should be rejected");
        };
        ensure!(
            error.kind() == ErrorKind::InvalidData,
            "invalid working directory returned {:?}, not InvalidData",
            error.kind()
        );
        Ok(())
    }
}
