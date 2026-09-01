//! Construction of the Ninja child command.
//!
//! Separated from `process::mod` so command shaping — working directory, job
//! count, build file, streaming pipes, and the injected environment — sits
//! together, and so neither module exceeds the repository's file-length limit.

use std::io;
use std::process::{Command, Stdio};

use camino::Utf8Path;

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
/// Returns an I/O error only when working-directory canonicalization fails.
/// Build and tool helpers tolerate build-file canonicalization failure by
/// passing the original UTF-8 path to Ninja.
fn configure_ninja_base(
    cmd: &mut Command,
    options: &NinjaProcessOptions,
    build_file: &Utf8Path,
    env: &CommandEnv,
) -> io::Result<()> {
    env.apply(cmd);
    if let Some(dir) = &options.working_dir {
        let canonical = canonicalize_utf8_path(dir)?;
        cmd.current_dir(canonical.as_std_path());
    }
    if let Some(jobs) = options.jobs {
        cmd.arg("-j").arg(jobs.to_string());
    }
    let build_file_path = canonicalize_utf8_path(build_file).unwrap_or_else(|_| build_file.into());
    cmd.arg("-f").arg(build_file_path.as_std_path());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    Ok(())
}

/// Configure a Ninja build command from `request`, appending its targets.
///
/// # Errors
///
/// Build-file canonicalization failure is tolerated by passing the original
/// UTF-8 path. Returns an error when working-directory canonicalization fails.
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
/// Build-file canonicalization failure is tolerated by passing the original
/// UTF-8 path. Returns an error when working-directory canonicalization fails.
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
    use anyhow::{Context, Result, ensure};
    use camino::{Utf8Path, Utf8PathBuf};
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

    fn expected_base_arguments(build_file: &Utf8Path) -> Result<Vec<OsString>> {
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
        let temp_build_file = build_file.into_temp_path();
        let utf8_build_file = Utf8PathBuf::from_path_buf(temp_build_file.to_path_buf())
            .map_err(|path| anyhow::anyhow!("tempfile path is not UTF-8: {}", path.display()))?;
        let resolved_options = options?;
        let target_names = vec![String::from("default")];
        let targets = super::super::BuildTargets::new(&target_names);
        let request = NinjaBuildRequest {
            program: Utf8Path::new("ninja"),
            options: &resolved_options,
            build_file: &utf8_build_file,
            targets: &targets,
            env: &env,
            stderr_mode: StderrMode::Forward,
        };
        let mut cmd = Command::new("ninja");

        configure_ninja_build_command(&mut cmd, &request)?;

        let mut expected = expected_base_arguments(&utf8_build_file)?;
        expected.push(OsString::from("default"));
        let actual = command_arguments(&cmd);
        ensure!(
            actual == expected,
            "build command argument order changed: {actual:?}"
        );
        Ok(())
    }

    #[test]
    fn build_configuration_uses_uncanonicalized_missing_build_file() -> Result<()> {
        let temporary_directory = tempfile::tempdir().context("create temporary directory")?;
        let utf8_temporary_directory = Utf8PathBuf::from_path_buf(
            temporary_directory.path().to_path_buf(),
        )
        .map_err(|path| anyhow::anyhow!("temporary directory is not UTF-8: {}", path.display()))?;
        let build_file = utf8_temporary_directory.join("missing-build.ninja");
        let options = NinjaProcessOptions::default();
        let target_names = Vec::new();
        let targets = super::super::BuildTargets::new(&target_names);
        let env = CommandEnv::inherit();
        let request = NinjaBuildRequest {
            program: Utf8Path::new("ninja"),
            options: &options,
            build_file: &build_file,
            targets: &targets,
            env: &env,
            stderr_mode: StderrMode::Forward,
        };
        let mut cmd = Command::new("ninja");

        ensure!(
            canonicalize_utf8_path(&build_file).is_err(),
            "the missing build file must not canonicalize"
        );
        configure_ninja_build_command(&mut cmd, &request)?;

        let expected = vec![
            OsString::from("-f"),
            build_file.into_std_path_buf().into_os_string(),
        ];
        let actual = command_arguments(&cmd);
        ensure!(
            actual == expected,
            "missing build file should be passed unchanged after -f: {actual:?}"
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
        let temp_build_file = build_file.into_temp_path();
        let utf8_build_file = Utf8PathBuf::from_path_buf(temp_build_file.to_path_buf())
            .map_err(|path| anyhow::anyhow!("tempfile path is not UTF-8: {}", path.display()))?;
        let resolved_options = options?;
        let request = NinjaToolRequest {
            program: Utf8Path::new("ninja"),
            options: &resolved_options,
            build_file: &utf8_build_file,
            tool: "clean",
            env: &env,
            stderr_mode: StderrMode::Forward,
        };
        let mut cmd = Command::new("ninja");

        configure_ninja_tool_command(&mut cmd, &request)?;

        let mut expected = expected_base_arguments(&utf8_build_file)?;
        expected.extend([OsString::from("-t"), OsString::from("clean")]);
        let actual = command_arguments(&cmd);
        ensure!(
            actual == expected,
            "tool command argument order changed: {actual:?}"
        );
        Ok(())
    }
}
