//! Typed-classification tests for Ninja invocation failures.
//!
//! These cases pin the distinction between a launch failure, an unsuccessful
//! exit, and any other process failure, because the runner chooses its
//! diagnostic from that classification rather than from an `io::ErrorKind`.

use super::*;
use rstest::{fixture, rstest};

#[cfg(unix)]
fn fake_ninja_script(script: &str) -> anyhow::Result<(tempfile::TempDir, camino::Utf8PathBuf)> {
    use test_support::exec::write_exec_with_content;

    let temp_dir = tempfile::tempdir()?;
    let program = write_exec_with_content(temp_dir.path(), "ninja", script)?;
    let utf8_program = camino::Utf8PathBuf::from_path_buf(program)
        .map_err(|path| anyhow::anyhow!("fake Ninja path is not UTF-8: {}", path.display()))?;
    Ok((temp_dir, utf8_program))
}

#[cfg(unix)]
fn fake_ninja_program() -> anyhow::Result<(tempfile::TempDir, std::path::PathBuf)> {
    let (temp_dir, program) = fake_ninja_script("#!/bin/sh\nexit 0\n")?;
    Ok((temp_dir, program.into_std_path_buf()))
}

/// Run one build with the supplied process settings and return its failure.
///
/// The build file must exist because command configuration canonicalises it; a
/// missing program then reaches `spawn` exactly as it would in a real run. The
/// request is assembled here rather than in a helper so the fixture stays
/// local to its callers.
#[cfg(unix)]
fn run_classification_build(
    program: &Utf8Path,
    build_file: &Utf8Path,
    options: &NinjaProcessOptions,
) -> anyhow::Result<NinjaInvocationError> {
    let targets = BuildTargets::default();
    let env = CommandEnv::inherit();
    let clock = FixedMonotonicClock::with_elapsed(Duration::ZERO);
    let request = NinjaBuildRequest {
        program,
        options,
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
#[fixture]
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
#[rstest]
fn missing_program_is_classified_as_a_spawn_failure(
    temporary_build_file: anyhow::Result<(tempfile::NamedTempFile, camino::Utf8PathBuf)>,
) -> anyhow::Result<()> {
    let program_dir = tempfile::tempdir()?;
    let program = camino::Utf8PathBuf::from_path_buf(program_dir.path().join("absent-ninja"))
        .map_err(|path| anyhow::anyhow!("tempdir path is not UTF-8: {}", path.display()))?;
    let (_build_lease, build_file) = temporary_build_file?;

    let error = run_classification_build(&program, &build_file, &NinjaProcessOptions::default())?;
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
    anyhow::ensure!(
        raw_os_error.is_some(),
        "a missing program should expose the OS error code"
    );

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
#[rstest]
fn unsuccessful_exit_is_classified_with_its_status(
    temporary_build_file: anyhow::Result<(tempfile::NamedTempFile, camino::Utf8PathBuf)>,
) -> anyhow::Result<()> {
    let (_program_dir, program) = fake_ninja_script("#!/bin/sh\nexit 7\n")?;
    let (_build_lease, build_file) = temporary_build_file?;

    let error = run_classification_build(&program, &build_file, &NinjaProcessOptions::default())?;
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
#[rstest]
fn signal_terminated_ninja_is_classified_as_an_exit_failure(
    temporary_build_file: anyhow::Result<(tempfile::NamedTempFile, camino::Utf8PathBuf)>,
) -> anyhow::Result<()> {
    use std::os::unix::process::ExitStatusExt;

    let (_program_dir, program) = fake_ninja_script("#!/bin/sh\nkill -TERM $$\n")?;
    let (_build_lease, build_file) = temporary_build_file?;

    let error = run_classification_build(&program, &build_file, &NinjaProcessOptions::default())?;
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

/// A configuration failure before spawn is an I/O failure, not a launch failure.
///
/// The missing-program case cannot protect this category, because it fails
/// inside `spawn`. An unusable working directory fails while the command is
/// configured, and must stay distinct so the runner does not blame the program.
#[cfg(unix)]
#[rstest]
fn configuration_failure_is_classified_as_io(
    temporary_build_file: anyhow::Result<(tempfile::NamedTempFile, camino::Utf8PathBuf)>,
) -> anyhow::Result<()> {
    let (_program_dir, program) = fake_ninja_script("#!/bin/sh\nexit 0\n")?;
    let (_build_lease, build_file) = temporary_build_file?;
    let absent_dir = tempfile::tempdir()?.path().join("absent");
    let working_dir = camino::Utf8PathBuf::from_path_buf(absent_dir).map_err(|path| {
        anyhow::anyhow!("working directory path is not UTF-8: {}", path.display())
    })?;
    let options = NinjaProcessOptions {
        working_dir: Some(working_dir),
        jobs: None,
    };

    let error = run_classification_build(&program, &build_file, &options)?;
    let NinjaInvocationError::Io(source) = &error else {
        anyhow::bail!("an unusable working directory should be an I/O failure, got {error:?}");
    };
    anyhow::ensure!(
        source.kind() == io::ErrorKind::NotFound,
        "a missing working directory should report NotFound, got {:?}",
        source.kind()
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn build_and_tool_execution_preserve_operation_labels(
    temporary_build_file: anyhow::Result<(tempfile::NamedTempFile, camino::Utf8PathBuf)>,
) -> anyhow::Result<()> {
    let (_program_dir, program) = fake_ninja_program()?;
    let (_build_lease, utf8_build_file) = temporary_build_file?;
    let utf8_program = camino::Utf8PathBuf::from_path_buf(program)
        .map_err(|path| anyhow::anyhow!("fake Ninja path is not UTF-8: {}", path.display()))?;
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
