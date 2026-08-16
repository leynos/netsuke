//! Focused tests for Ninja exit diagnostics and execution operation labels.

use super::child_exit::check_exit_status_with_context;
use super::*;
use crate::test_tracing_capture::with_test_subscriber;
use monotony::test_util::FixedMonotonicClock;
use std::{
    process::ExitStatus,
    time::{Duration, Instant},
};
use tracing_subscriber::filter::LevelFilter;

#[cfg(unix)]
fn exit_status(code: i32) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;

    ExitStatus::from_raw(code << 8)
}

#[cfg(windows)]
fn exit_status(code: i32) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;

    ExitStatus::from_raw(code as u32)
}

fn command_log_context() -> CommandLogContext {
    CommandLogContext::from_command(&Command::new("ninja"))
}

fn captured_exit_result(status: ExitStatus) -> (io::Result<()>, Vec<String>) {
    let clock = FixedMonotonicClock::with_elapsed(Duration::ZERO);
    let context = command_log_context();
    let failure_context = ExitFailureContext {
        operation: "clean",
        stderr_mode: StderrMode::Suppress,
        command_list_failure: None,
        clock: &clock,
        started_at: Instant::now(),
    };
    with_test_subscriber(LevelFilter::WARN, |captured| {
        let result = check_exit_status_with_context(status, &context, &failure_context);
        (result, captured.snapshot())
    })
}

#[test]
fn successful_ninja_exit_returns_ok_without_exit_failure_event() {
    let (result, events) = captured_exit_result(exit_status(0));

    assert!(result.is_ok(), "a successful Ninja exit should succeed");
    assert!(
        events.is_empty(),
        "a successful Ninja exit should emit no exit-failure warning: {events:?}"
    );
}

#[test]
fn failed_ninja_exit_records_exit_status_diagnostics() {
    let (result, events) = captured_exit_result(exit_status(1));

    assert!(
        result.is_err(),
        "a failed Ninja exit should return an error"
    );
    let [event] = events.as_slice() else {
        panic!("expected one exit-failure event, got {events:?}");
    };
    assert!(
        event.contains("operation=\"clean\""),
        "exit failure should retain the operation label: {event}"
    );
    assert!(
        event.contains("failure_category=\"exit_status\""),
        "exit failure should identify its category: {event}"
    );
    assert!(
        event.contains("status="),
        "exit failure should include the child status: {event}"
    );
}

#[cfg(unix)]
fn fake_ninja_program() -> anyhow::Result<(tempfile::TempDir, std::path::PathBuf)> {
    use cap_std::{
        ambient_authority,
        fs::{Dir, PermissionsExt},
    };

    let temp_dir = tempfile::tempdir()?;
    let directory = Dir::open_ambient_dir(temp_dir.path(), ambient_authority())?;
    directory.write("ninja", "#!/bin/sh\nexit 0\n")?;
    let mut permissions = directory.metadata("ninja")?.permissions();
    permissions.set_mode(0o700);
    directory.set_permissions("ninja", permissions)?;
    let program = temp_dir.path().join("ninja");
    Ok((temp_dir, program))
}

#[cfg(unix)]
#[test]
fn build_and_tool_execution_preserve_operation_labels() -> anyhow::Result<()> {
    let (_program_dir, program) = fake_ninja_program()?;
    let build_file = tempfile::NamedTempFile::new()?;
    let options = NinjaProcessOptions::default();
    let env = CommandEnv::inherit();
    let target_names = vec![String::from("default")];
    let targets = BuildTargets::new(&target_names);
    let clock = FixedMonotonicClock::with_elapsed(Duration::ZERO);
    let (build_result, build_events) = with_test_subscriber(LevelFilter::INFO, |captured| {
        let result = run_ninja_build_internal(
            NinjaBuildRequest {
                program: &program,
                options: &options,
                build_file: build_file.path(),
                targets: &targets,
                env: &env,
                stderr_mode: StderrMode::Forward,
            },
            None,
            &clock,
        );
        (result, captured.snapshot())
    });
    build_result?;
    anyhow::ensure!(
        build_events
            .iter()
            .any(|event| event.contains("operation=\"build\"")),
        "build execution should log the build operation: {build_events:?}"
    );

    let (tool_result, tool_events) = with_test_subscriber(LevelFilter::INFO, |captured| {
        let result = run_ninja_tool_internal(
            NinjaToolRequest {
                program: &program,
                options: &options,
                build_file: build_file.path(),
                tool: "clean",
                env: &env,
                stderr_mode: StderrMode::Forward,
            },
            None,
            &clock,
        );
        (result, captured.snapshot())
    });
    tool_result?;
    anyhow::ensure!(
        tool_events
            .iter()
            .any(|event| event.contains("operation=\"clean\"")),
        "tool execution should log the request tool: {tool_events:?}"
    );
    Ok(())
}
