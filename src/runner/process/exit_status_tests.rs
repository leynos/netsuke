//! Focused tests for Ninja exit diagnostics and execution operation labels.

use super::child_exit::check_exit_status_with_context;
use super::test_fixtures::{
    TEST_ACTION_ID, add_target_edge, graph_with_identity_action as build_identity_graph,
};
use super::*;
use crate::test_tracing_capture::with_test_subscriber;
use crate::{
    ir::BuildGraph,
    ninja_gen::command_list::{COMMAND_LIST_FAILURE_PREFIX, action_identity},
};
use monotony::test_util::FixedMonotonicClock;
use rstest::{fixture, rstest};
use std::{
    io::Write,
    process::ExitStatus,
    time::{Duration, Instant},
};
use tracing_subscriber::filter::LevelFilter;

/// Build the graph fixture shared with command-list target-index tests.
#[fixture]
fn graph_with_identity_action() -> BuildGraph {
    build_identity_graph()
}

/// Parse one command-list marker into the failure value used by exit handling.
fn command_list_failure(fingerprint: &str) -> super::failure_attribution::CommandListFailure {
    let mut writer = super::failure_attribution::FailureAttributionWriter::new(Vec::new());
    let marker = format!("{COMMAND_LIST_FAILURE_PREFIX}{fingerprint}, entry 2\n");
    writer
        .write_all(marker.as_bytes())
        .expect("test marker should be writable");
    writer
        .into_failure()
        .expect("test marker should parse into an attribution")
}

#[cfg(unix)]
fn exit_status(code: i32) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;

    ExitStatus::from_raw(code << 8)
}

#[cfg(windows)]
fn exit_status(code: u32) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;

    ExitStatus::from_raw(code)
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
        command_list_targets: None,
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

#[rstest]
#[case::unique(
    &["check-fmt"],
    TEST_ACTION_ID,
    Some("target \"check-fmt\"")
)]
#[case::ambiguous(
    &["lint", "check-fmt"],
    TEST_ACTION_ID,
    Some("targets \"check-fmt\", \"lint\"")
)]
#[case::unknown(
    &["check-fmt"],
    "unindexed-action",
    None
)]
fn exit_failure_uses_one_target_resolution_for_error_and_tracing(
    mut graph_with_identity_action: BuildGraph,
    #[case] targets: &[&str],
    #[case] marker_action_id: &str,
    #[case] expected_clause: Option<&str>,
) {
    for target in targets {
        add_target_edge(&mut graph_with_identity_action, target);
    }
    let index = super::command_list_targets::CommandListTargetIndex::from_graph(
        &graph_with_identity_action,
    );
    let fingerprint = action_identity(marker_action_id);
    let failure = command_list_failure(&fingerprint);
    let clock = FixedMonotonicClock::with_elapsed(Duration::ZERO);
    let context = command_log_context();
    let failure_context = ExitFailureContext {
        operation: "build",
        stderr_mode: StderrMode::Suppress,
        command_list_failure: Some(&failure),
        command_list_targets: Some(&index),
        clock: &clock,
        started_at: Instant::now(),
    };
    let (result, events) = with_test_subscriber(LevelFilter::WARN, |captured| {
        let result = check_exit_status_with_context(exit_status(1), &context, &failure_context);
        (result, captured.snapshot())
    });

    let error = result.expect_err("a failing Ninja exit should return an error");
    let marker = format!("{COMMAND_LIST_FAILURE_PREFIX}{fingerprint}, entry 2");
    let error_text = error.to_string();
    assert!(
        error_text.contains(&marker),
        "the error should retain the full failure marker: {error_text}"
    );
    let telemetry = events
        .iter()
        .find(|event| event.contains("command_list_action="))
        .expect("the attributed failure should emit its tracing event");
    assert!(
        telemetry.contains(&format!("command_list_action={fingerprint:?}")),
        "tracing should retain the full action fingerprint: {telemetry}"
    );

    if let Some(clause) = expected_clause {
        assert!(
            error_text.contains(&format!("({clause})")),
            "the error should append the resolved target clause: {error_text}"
        );
        assert!(
            telemetry.contains(&format!("command_list_target={clause:?}")),
            "tracing should record the same resolved target clause: {telemetry}"
        );
    } else {
        assert!(
            !error_text.contains("command_list_target") && !error_text.contains("target \""),
            "an unknown fingerprint should leave the error unchanged: {error_text}"
        );
        assert_eq!(
            error_text,
            format!("ninja exited with {}: {marker}", exit_status(1)),
            "an unknown fingerprint should preserve the exact legacy error text"
        );
        assert!(
            !telemetry.contains("command_list_target="),
            "an unknown fingerprint should omit the tracing field: {telemetry}"
        );
    }
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
    let utf8_program = camino::Utf8PathBuf::from_path_buf(program)
        .map_err(|path| anyhow::anyhow!("fake Ninja path is not UTF-8: {}", path.display()))?;
    let utf8_build_file =
        camino::Utf8PathBuf::from_path_buf(build_file.into_temp_path().to_path_buf())
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
    tool_result?;
    anyhow::ensure!(
        tool_events
            .iter()
            .any(|event| event.contains("operation=\"clean\"")),
        "tool execution should log the request tool: {tool_events:?}"
    );
    Ok(())
}
