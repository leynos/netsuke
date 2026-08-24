//! Construction of the Ninja child command.
//!
//! Separated from `process::mod` so command shaping — working directory, job
//! count, build file, streaming pipes, and the injected environment — sits
//! together, and so neither module exceeds the repository's file-length limit.

use std::io::{self, ErrorKind};
use std::path::Path;
use std::process::{Command, Stdio};

use camino::Utf8PathBuf;

use super::{
    CommandEnv, NinjaBuildRequest, NinjaProcessOptions, NinjaToolRequest, canonicalize_utf8_path,
};

/// Configure the base Ninja command with working directory, job count, and build file.
///
/// Sets up stdout/stderr pipes for streaming. Callers append targets or tool
/// flags after this function returns.
///
/// # Errors
///
/// Returns an I/O error only when working-directory canonicalization fails or
/// the build-file path remains non-UTF-8 after the fallback. Build and tool
/// helpers tolerate build-file canonicalization failure when the original path
/// is valid UTF-8.
fn configure_ninja_base(
    cmd: &mut Command,
    options: &NinjaProcessOptions,
    build_file: &Path,
    env: &CommandEnv,
) -> io::Result<()> {
    env.apply(cmd);
    if let Some(dir) = &options.working_dir {
        let canonical = canonicalize_utf8_path(dir.as_std_path())?;
        cmd.current_dir(canonical.as_std_path());
    }
    if let Some(jobs) = options.jobs {
        cmd.arg("-j").arg(jobs.to_string());
    }
    let build_file_path = canonicalize_utf8_path(build_file).or_else(|_| {
        Utf8PathBuf::from_path_buf(build_file.to_path_buf()).map_err(|_| {
            io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "build file path {} is not valid UTF-8",
                    build_file.display()
                ),
            )
        })
    })?;
    cmd.arg("-f").arg(build_file_path.as_std_path());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    Ok(())
}

/// Configure a Ninja build command from `request`, appending its targets.
///
/// # Errors
///
/// Returns an error when the working directory or build file cannot be
/// canonicalized.
pub(super) fn configure_ninja_build_command(
    cmd: &mut Command,
    request: &NinjaBuildRequest<'_>,
) -> io::Result<()> {
    configure_ninja_base(cmd, request.options, request.build_file, request.env)?;
    let targets = request.targets;
    cmd.args(targets.as_slice());
    Ok(())
}

/// Configure a Ninja tool command from `request`, appending the tool name.
///
/// # Errors
///
/// Returns an error when the working directory or build file cannot be
/// canonicalized.
pub(super) fn configure_ninja_tool_command(
    cmd: &mut Command,
    request: &NinjaToolRequest<'_>,
) -> io::Result<()> {
    configure_ninja_base(cmd, request.options, request.build_file, request.env)?;
    cmd.arg("-t").arg(request.tool);
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Unit tests for Ninja command configuration without spawning Ninja.

    use super::*;
    use crate::runner::{NinjaJobCount, StderrMode};
    use anyhow::{Result, ensure};
    use rstest::{fixture, rstest};
    use std::ffi::{OsStr, OsString};
    use tempfile::NamedTempFile;

    #[fixture]
    fn temp_file() -> Result<NamedTempFile> {
        Ok(NamedTempFile::new()?)
    }

    #[fixture]
    fn options() -> io::Result<NinjaProcessOptions> {
        Ok(NinjaProcessOptions {
            jobs: Some(NinjaJobCount::try_new(4)?),
            ..NinjaProcessOptions::default()
        })
    }

    #[fixture]
    fn env() -> CommandEnv {
        CommandEnv::inherit()
    }

    fn command_arguments(cmd: &Command) -> Vec<OsString> {
        cmd.get_args().map(OsStr::to_os_string).collect()
    }

    fn expected_base_arguments(build_file: &Path) -> Result<Vec<OsString>> {
        Ok(vec![
            OsString::from("-j"),
            OsString::from("4"),
            OsString::from("-f"),
            canonicalize_utf8_path(build_file)?
                .into_std_path_buf()
                .into_os_string(),
        ])
    }

    #[rstest]
    fn build_configuration_preserves_argument_order(
        temp_file: Result<NamedTempFile>,
        options: io::Result<NinjaProcessOptions>,
        env: CommandEnv,
    ) -> Result<()> {
        let build_file = temp_file?;
        let resolved_options = options?;
        let target_names = vec![String::from("default")];
        let targets = super::super::BuildTargets::new(&target_names);
        let request = NinjaBuildRequest {
            program: Path::new("ninja"),
            options: &resolved_options,
            build_file: build_file.path(),
            targets: &targets,
            env: &env,
            stderr_mode: StderrMode::Forward,
        };
        let mut cmd = Command::new("ninja");

        configure_ninja_build_command(&mut cmd, &request)?;

        let mut expected = expected_base_arguments(build_file.path())?;
        expected.push(OsString::from("default"));
        let actual = command_arguments(&cmd);
        ensure!(
            actual == expected,
            "build command argument order changed: {actual:?}"
        );
        Ok(())
    }

    #[rstest]
    fn tool_configuration_preserves_argument_order(
        temp_file: Result<NamedTempFile>,
        options: io::Result<NinjaProcessOptions>,
        env: CommandEnv,
    ) -> Result<()> {
        let build_file = temp_file?;
        let resolved_options = options?;
        let request = NinjaToolRequest {
            program: Path::new("ninja"),
            options: &resolved_options,
            build_file: build_file.path(),
            tool: "clean",
            env: &env,
            stderr_mode: StderrMode::Forward,
        };
        let mut cmd = Command::new("ninja");

        configure_ninja_tool_command(&mut cmd, &request)?;

        let mut expected = expected_base_arguments(build_file.path())?;
        expected.extend([OsString::from("-t"), OsString::from("clean")]);
        let actual = command_arguments(&cmd);
        ensure!(
            actual == expected,
            "tool command argument order changed: {actual:?}"
        );
        Ok(())
    }
}
