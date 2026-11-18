//! Typed-classification tests for Ninja invocation failures.
//!
//! These cases pin the distinction between a launch failure, an unsuccessful
//! exit, and any other process failure, because the runner chooses its
//! diagnostic from that classification rather than from an `io::ErrorKind`.

use super::*;

#[cfg(unix)]
fn fake_ninja_script(script: &str) -> anyhow::Result<(tempfile::TempDir, camino::Utf8PathBuf)> {
    use cap_std::{
        ambient_authority,
        fs::{Dir, PermissionsExt},
    };

    let temp_dir = tempfile::tempdir()?;
    let directory = Dir::open_ambient_dir(temp_dir.path(), ambient_authority())?;
    directory.write("ninja", script)?;
    let mut permissions = directory.metadata("ninja")?.permissions();
    permissions.set_mode(0o700);
    directory.set_permissions("ninja", permissions)?;
    let program = camino::Utf8PathBuf::from_path_buf(temp_dir.path().join("ninja"))
        .map_err(|path| anyhow::anyhow!("fake Ninja path is not UTF-8: {}", path.display()))?;
    Ok((temp_dir, program))
}

#[cfg(unix)]
fn fake_ninja_program() -> anyhow::Result<(tempfile::TempDir, std::path::PathBuf)> {
    let (temp_dir, program) = fake_ninja_script("#!/bin/sh\nexit 0\n")?;
    Ok((temp_dir, program.into_std_path_buf()))
}

/// Run one build and return the typed failure it produced.
///
/// The build file must exist because command configuration canonicalises it; a
/// missing program then reaches `spawn` exactly as it would in a real run. The
/// request is assembled here rather than in a helper so the fixture stays
/// local to its single caller.
#[cfg(unix)]
fn run_classification_build(
    program: &Utf8Path,
    build_file: &Utf8Path,
) -> anyhow::Result<NinjaInvocationError> {
    let options = NinjaProcessOptions::default();
    let targets = BuildTargets::default();
    let env = CommandEnv::inherit();
    let clock = FixedMonotonicClock::with_elapsed(Duration::ZERO);
    let request = NinjaBuildRequest {
        program,
        options: &options,
        build_file,
        targets: &targets,
        env: &env,
        stderr_mode: StderrMode::Suppress,
    };
    run_ninja_build_internal(request, None, &clock)
        .err()
        .ok_or_else(|| anyhow::anyhow!("the fake Ninja should have failed"))
}

/// Build an existing temporary build file whose path stays UTF-8.
///
/// The file owner is returned beside the path because dropping it deletes the
/// file. Callers must bind it for at least as long as they use the path, since
/// command configuration canonicalises an existing build file.
#[cfg(unix)]
fn temporary_build_file() -> anyhow::Result<(tempfile::NamedTempFile, camino::Utf8PathBuf)> {
    let file = tempfile::NamedTempFile::new()?;
    let path = camino::Utf8PathBuf::from_path_buf(file.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("build file path is not UTF-8: {}", path.display()))?;
    Ok((file, path))
}

/// A launch failure is classified as such and keeps the OS classification.
///
/// The public conversion must not lose `NotFound` or the raw OS code, because
/// existing callers branch on those values.
#[cfg(unix)]
#[test]
fn missing_program_is_classified_as_a_spawn_failure() -> anyhow::Result<()> {
    let program_dir = tempfile::tempdir()?;
    let program = camino::Utf8PathBuf::from_path_buf(program_dir.path().join("absent-ninja"))
        .map_err(|path| anyhow::anyhow!("tempdir path is not UTF-8: {}", path.display()))?;
    let (_build_file, build_file) = temporary_build_file()?;

    let error = run_classification_build(&program, &build_file)?;
    let NinjaInvocationError::Spawn {
        program: attempted,
        source,
    } = &error
    else {
        anyhow::bail!("a missing program should be a spawn failure, got {error:?}");
    };
    anyhow::ensure!(
        attempted == &program,
        "the failure should name the attempted program: {attempted}"
    );
    anyhow::ensure!(
        source.kind() == io::ErrorKind::NotFound,
        "a missing program should report NotFound, got {:?}",
        source.kind()
    );
    let raw_os_error = source.raw_os_error();

    let converted = error.into_io_error();
    anyhow::ensure!(
        converted.kind() == io::ErrorKind::NotFound,
        "the public error should keep NotFound, got {:?}",
        converted.kind()
    );
    anyhow::ensure!(
        converted.raw_os_error() == raw_os_error,
        "the public error should keep the OS code: {:?} became {:?}",
        raw_os_error,
        converted.raw_os_error()
    );
    Ok(())
}

/// An ordinary nonzero exit is classified as an exit failure with its status.
#[cfg(unix)]
#[test]
fn unsuccessful_exit_is_classified_with_its_status() -> anyhow::Result<()> {
    let (_program_dir, program) = fake_ninja_script("#!/bin/sh\nexit 7\n")?;
    let (_build_file, build_file) = temporary_build_file()?;

    let error = run_classification_build(&program, &build_file)?;
    let NinjaInvocationError::Exit(exit) = &error else {
        anyhow::bail!("an unsuccessful exit should be an exit failure, got {error:?}");
    };
    anyhow::ensure!(
        exit.status().code() == Some(7),
        "the status should carry Ninja's exit code: {:?}",
        exit.status()
    );
    anyhow::ensure!(
        exit.operation() == "build",
        "the failure should keep the build operation label, got {}",
        exit.operation()
    );

    let legacy = error.into_io_error().to_string();
    anyhow::ensure!(
        legacy == format!("ninja exited with {}", exit_status(7)),
        "the public error should preserve the legacy text: {legacy}"
    );
    Ok(())
}

/// A signal-terminated Ninja is still an exit failure, and keeps its signal.
#[cfg(unix)]
#[test]
fn signal_terminated_ninja_is_classified_as_an_exit_failure() -> anyhow::Result<()> {
    use std::os::unix::process::ExitStatusExt;

    let (_program_dir, program) = fake_ninja_script("#!/bin/sh\nkill -TERM $$\n")?;
    let (_build_file, build_file) = temporary_build_file()?;

    let error = run_classification_build(&program, &build_file)?;
    let NinjaInvocationError::Exit(exit) = &error else {
        anyhow::bail!("a terminated Ninja should be an exit failure, got {error:?}");
    };
    anyhow::ensure!(
        exit.status().signal() == Some(15),
        "the status should keep the terminating signal: {:?}",
        exit.status()
    );
    anyhow::ensure!(
        exit.status().code().is_none(),
        "a signal-terminated status has no exit code: {:?}",
        exit.status()
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn build_and_tool_execution_preserve_operation_labels() -> anyhow::Result<()> {
    let (_program_dir, program) = fake_ninja_program()?;
    let build_file = tempfile::NamedTempFile::new()?;
    let utf8_program = camino::Utf8PathBuf::from_path_buf(program)
        .map_err(|path| anyhow::anyhow!("fake Ninja path is not UTF-8: {}", path.display()))?;
    // `build_file` stays bound so the file outlives the path derived from it.
    let utf8_build_file = camino::Utf8PathBuf::from_path_buf(build_file.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("build file path is not UTF-8: {}", path.display()))?;
    let options = NinjaProcessOptions::default();
    let env = CommandEnv::inherit();
    let target_names = vec![String::from("default")];
    let targets = BuildTargets::new(&target_names);
    let clock = FixedMonotonicClock::with_elapsed(Duration::ZERO);
    let (build_result, build_events) = with_test_subscriber(LevelFilter::INFO, |captured| {
        let result = run_ninja_build_internal(
            NinjaBuildRequest {
                program: &utf8_program,
                options: &options,
                build_file: &utf8_build_file,
                targets: &targets,
                env: &env,
                stderr_mode: StderrMode::Forward,
            },
            None,
            &clock,
        );
        (result, captured.snapshot())
    });
    build_result.map_err(NinjaInvocationError::into_io_error)?;
    anyhow::ensure!(
        build_events
            .iter()
            .any(|event| event.contains("operation=\"build\"")),
        "build execution should log the build operation: {build_events:?}"
    );

    let (tool_result, tool_events) = with_test_subscriber(LevelFilter::INFO, |captured| {
        let result = run_ninja_tool_internal(
            NinjaToolRequest {
                program: &utf8_program,
                options: &options,
                build_file: &utf8_build_file,
                tool: "clean",
                env: &env,
                stderr_mode: StderrMode::Forward,
            },
            None,
            &clock,
        );
        (result, captured.snapshot())
    });
    tool_result.map_err(NinjaInvocationError::into_io_error)?;
    anyhow::ensure!(
        tool_events
            .iter()
            .any(|event| event.contains("operation=\"clean\"")),
        "tool execution should log the request tool: {tool_events:?}"
    );
    Ok(())
}
