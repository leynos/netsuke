//! Behavioural tracing tests for public Ninja process execution.

use super::{
    BuildTargets, CommandEnv, NinjaBuildRequest, NinjaProcessOptions, StderrMode, run_ninja_with,
};
use crate::test_tracing_capture::with_test_subscriber;
use std::collections::{BTreeMap, BTreeSet};
use tracing_subscriber::filter::LevelFilter;

/// Run a successful public Ninja build request and capture its tracing events.
///
/// # Errors
///
/// Returns an error when the temporary manifest or executable cannot be
/// created, or when the public build request fails.
fn capture_public_ninja_execution(level_filter: LevelFilter) -> anyhow::Result<Vec<String>> {
    use test_support::exec::write_exec_with_content;

    let workspace = tempfile::tempdir()?;
    let build_file = workspace.path().join("build.ninja");
    test_support::fs::write(&build_file, "# empty manifest\n")?;
    let fake_ninja =
        write_exec_with_content(workspace.path(), "fake-ninja", "#!/bin/sh\nexit 0\n")?;
    let options = NinjaProcessOptions::default();
    let targets = BuildTargets::default();
    let env = CommandEnv::inherit();
    let (result, events) = with_test_subscriber(level_filter, |captured| {
        let result = run_ninja_with(&NinjaBuildRequest {
            program: &fake_ninja,
            options: &options,
            build_file: &build_file,
            targets: &targets,
            env: &env,
            stderr_mode: StderrMode::Forward,
        });
        (result, captured.snapshot())
    });
    result?;
    Ok(events)
}

/// Verify that `event` contains every stable field required by its contract.
///
/// # Errors
///
/// Returns an error identifying the missing field when the event violates its
/// tracing contract.
fn assert_event_fields(
    event: &str,
    required_fields: &[&str],
    event_kind: &str,
) -> anyhow::Result<()> {
    for field in required_fields {
        anyhow::ensure!(
            event.contains(field),
            "{event_kind} event should contain {field:?}: {event}"
        );
    }
    Ok(())
}

/// Parse the space-separated field rendering produced by the test subscriber.
///
/// Values may contain spaces, so fragments without an equals sign extend the
/// preceding field value.
fn parse_event_fields(event: &str) -> BTreeMap<&str, String> {
    let mut fields = BTreeMap::new();
    let mut active_field = None;

    for fragment in event.split_whitespace() {
        match fragment.split_once('=') {
            Some((field, value)) => {
                fields.insert(field, value.to_owned());
                active_field = Some(field);
            }
            None => active_field.into_iter().for_each(|field| {
                append_event_field_fragment(&mut fields, field, fragment);
            }),
        }
    }

    fields
}

/// Append a space-separated continuation fragment to a captured field value.
fn append_event_field_fragment<'event>(
    fields: &mut BTreeMap<&'event str, String>,
    field: &'event str,
    fragment: &str,
) {
    fields.entry(field).and_modify(|value| {
        value.push(' ');
        value.push_str(fragment);
    });
}

/// Verify the stable informational event emitted by public Ninja execution.
///
/// # Errors
///
/// Returns an error when public execution emits any number other than one
/// informational event, includes a required field, or exposes the command.
fn assert_public_ninja_info_event(events: &[String]) -> anyhow::Result<()> {
    let [event] = events else {
        anyhow::bail!("expected one informational event, got {events:?}");
    };
    assert_event_fields(
        event,
        &[
            "message=Executing Ninja subprocess",
            "operation=\"build\"",
            "ninja_program=",
            "arg_count=",
            "env_override_count=0",
            "path_overridden=false",
            "suppress_stderr=false",
        ],
        "informational",
    )?;
    let fields = parse_event_fields(event);
    let expected_field_names = BTreeSet::from([
        "arg_count",
        "env_override_count",
        "message",
        "ninja_program",
        "operation",
        "path_overridden",
        "suppress_stderr",
    ]);
    let actual_field_names = fields.keys().copied().collect::<BTreeSet<_>>();
    anyhow::ensure!(
        actual_field_names == expected_field_names,
        "informational event fields must be exactly {expected_field_names:?}, got {actual_field_names:?}: {event}"
    );
    anyhow::ensure!(
        fields
            .get("message")
            .is_some_and(|message| message == "Executing Ninja subprocess"),
        "informational event must use the exact static message, got {:?}: {event}",
        fields.get("message")
    );
    anyhow::ensure!(
        !event.contains("Executing command"),
        "informational event must not contain the redacted command: {event}"
    );
    anyhow::ensure!(
        !fields.contains_key("redacted_command"),
        "informational event must not include a redacted_command field: {event}"
    );
    Ok(())
}

/// Verify the verbose command event emitted by public Ninja execution.
///
/// # Errors
///
/// Returns an error when public execution omits the debug command event or it
/// does not retain the required correlation and command fields.
fn assert_public_ninja_debug_event(events: &[String]) -> anyhow::Result<()> {
    let Some(event) = events
        .iter()
        .find(|event| event.contains("message=Executing command:"))
    else {
        anyhow::bail!("expected a debug command event, got {events:?}");
    };
    assert_event_fields(
        event,
        &[
            "operation=\"build\"",
            "ninja_program=",
            "suppress_stderr=false",
            "Executing command:",
            "fake-ninja",
        ],
        "debug command",
    )
}

/// Verify public Ninja execution separates stable and verbose event payloads.
#[test]
fn public_ninja_execution_splits_info_and_debug_events() -> anyhow::Result<()> {
    let info_events = capture_public_ninja_execution(LevelFilter::INFO)?;
    assert_public_ninja_info_event(&info_events)?;

    let debug_events = capture_public_ninja_execution(LevelFilter::DEBUG)?;
    assert_public_ninja_debug_event(&debug_events)
}
