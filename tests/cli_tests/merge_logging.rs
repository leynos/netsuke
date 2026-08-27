//! Structured debug-logging tests for the configuration merge pipeline.
//!
//! Verifies explicit merge observability with structured layer and validation
//! fields while keeping caller-controlled paths and values out of events.

use anyhow::{Context, Result, ensure};
use netsuke::cli::{MergeEvent, MergeObserver};
use rstest::rstest;
use std::{ffi::OsString, sync::Arc};
use test_support::tracing_capture::with_test_subscriber;
use tracing_subscriber::filter::LevelFilter;

#[derive(Default)]
struct TestEnv {
    entries: Vec<(OsString, OsString)>,
}

impl netsuke::cli::ConfigEnvProvider for TestEnv {
    fn get(&self, _key: &str) -> Option<OsString> {
        None
    }

    fn entries(&self) -> Vec<(OsString, OsString)> {
        self.entries.clone()
    }
}

/// Collect bounded merge events without installing a tracing subscriber.
#[derive(Default)]
struct EventCollector {
    events: Vec<MergeEvent>,
}

impl MergeObserver for EventCollector {
    fn observe(&mut self, event: MergeEvent) {
        self.events.push(event);
    }
}

/// Run the cached merge with a caller-owned observer.
fn merge_and_observe(cli_args: &[&str], env: &TestEnv) -> Result<(Vec<MergeEvent>, bool)> {
    let localizer = Arc::from(netsuke::cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(cli_args, &localizer)
        .context("parse CLI args for merge observer test")?;
    let (json_mode, outcome) =
        netsuke::cli::resolve_json_and_layers_outcome_with_env(&cli, &matches, env);
    json_mode.context("resolve diagnostic mode before merge")?;
    let input = netsuke::cli::CachedMergeInput::new(&cli, &matches, env, outcome.into_layers());
    let mut observer = EventCollector::default();
    let merge_ok =
        netsuke::cli::merge_with_cached_file_layers_with_observer(input, &mut observer).is_ok();
    Ok((observer.events, merge_ok))
}

/// Run the cached merge with the application's explicit tracing adapter.
fn merge_and_capture(cli_args: &[&str], env: &TestEnv) -> Result<(Vec<String>, bool)> {
    let localizer = Arc::from(netsuke::cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(cli_args, &localizer)
        .context("parse CLI args for merge logging test")?;
    let (json_mode, outcome) =
        netsuke::cli::resolve_json_and_layers_outcome_with_env(&cli, &matches, env);
    json_mode.context("resolve diagnostic mode before merge")?;
    Ok(with_test_subscriber(LevelFilter::DEBUG, |captured| {
        let mut observer = netsuke::cli::TracingMergeObserver;
        let input = netsuke::cli::CachedMergeInput::new(&cli, &matches, env, outcome.into_layers());
        let merge_ok =
            netsuke::cli::merge_with_cached_file_layers_with_observer(input, &mut observer).is_ok();
        (captured.snapshot(), merge_ok)
    }))
}

/// Assert that at least one captured event contains a structured field fragment.
fn assert_contains(events: &[String], needle: &str) -> Result<()> {
    ensure!(
        events.iter().any(|event| event.contains(needle)),
        "expected a captured event containing {needle:?}; got {events:#?}"
    );
    Ok(())
}

#[rstest]
fn merge_query_without_observer_emits_no_merge_events() -> Result<()> {
    let localizer = Arc::from(netsuke::cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer)
        .context("parse CLI args for side-effect-free merge test")?;
    let events = with_test_subscriber(LevelFilter::DEBUG, |captured| {
        let merge = netsuke::cli::merge_with_config_and_env(&cli, &matches, &TestEnv::default());
        ensure!(
            merge.is_ok(),
            "plain merge should succeed without an observer"
        );
        Ok::<_, anyhow::Error>(captured.snapshot())
    })?;

    ensure!(
        events.iter().all(|event| !event.contains("layer=\"")),
        "merge queries without an observer must not emit merge events: {events:#?}"
    );
    Ok(())
}

#[rstest]
fn observer_reports_exact_empty_input_events() -> Result<()> {
    let directory = tempfile::tempdir().context("create empty configuration directory")?;
    let directory_arg = directory.path().to_string_lossy().into_owned();
    let (events, merge_ok) = merge_and_observe(
        &["netsuke", "--directory", &directory_arg],
        &TestEnv::default(),
    )?;

    ensure!(
        merge_ok,
        "merge should succeed with empty configuration inputs"
    );
    ensure!(
        matches!(
            events.as_slice(),
            [
                MergeEvent::DefaultsApplied,
                MergeEvent::FileLayersCollected { layer_count: 0 },
                MergeEvent::EnvironmentApplied { is_empty: true },
                MergeEvent::CliOverridesAbsent,
            ]
        ),
        "empty inputs should produce the bounded event sequence: {events:#?}"
    );
    Ok(())
}

#[rstest]
fn merge_emits_debug_event_per_layer() -> Result<()> {
    let (events, merge_ok) = merge_and_capture(&["netsuke"], &TestEnv::default())?;
    ensure!(merge_ok, "merge should succeed for plain invocation");
    assert_contains(&events, "layer=\"defaults\"")?;
    assert_contains(&events, "layer=\"file\"")?;
    assert_contains(&events, "layer=\"environment\"")?;
    assert_contains(&events, "layer=\"cli\"")?;
    Ok(())
}

#[rstest]
fn merge_logs_explicit_cli_override_keys() -> Result<()> {
    let private_host = "private-host.example";
    let (events, merge_ok) = merge_and_capture(
        &["netsuke", "--fetch-allow-host", private_host],
        &TestEnv::default(),
    )?;
    ensure!(merge_ok, "merge should succeed with CLI override");
    assert_contains(&events, "override_keys")?;
    assert_contains(&events, "fetch_allow_host")?;
    ensure!(
        events.iter().all(|event| !event.contains(private_host)),
        "CLI override values must not be recorded: {events:#?}"
    );
    Ok(())
}

#[rstest]
fn merge_logs_nested_cli_override_leaf_paths() -> Result<()> {
    let target = "private-release-target";
    let (events, merge_ok) = merge_and_capture(&["netsuke", "build", target], &TestEnv::default())?;
    ensure!(
        merge_ok,
        "merge should succeed with a build target override"
    );
    assert_contains(&events, "cmds.build.targets")?;
    ensure!(
        events.iter().all(|event| !event.contains(target)),
        "nested CLI values must not be recorded: {events:#?}"
    );
    Ok(())
}

#[rstest]
fn merge_logs_validation_rejection_with_key_and_reason() -> Result<()> {
    let temp_dir = tempfile::tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    test_support::fs::write(&config_path, "jobs = 0\n").context("write netsuke.toml")?;

    let localizer = Arc::from(netsuke::cli_localization::build_localizer(None));
    let config_arg = config_path.to_string_lossy().into_owned();
    let (cli, matches) =
        netsuke::cli::parse_with_localizer_from(["netsuke", "--config", &config_arg], &localizer)
            .context("parse CLI args")?;
    let env = TestEnv::default();
    let (json_mode, outcome) =
        netsuke::cli::resolve_json_and_layers_outcome_with_env(&cli, &matches, &env);
    json_mode.context("resolve diagnostic mode before merge")?;
    let (events, merge_ok) = with_test_subscriber(LevelFilter::DEBUG, |captured| {
        let mut observer = netsuke::cli::TracingMergeObserver;
        let input =
            netsuke::cli::CachedMergeInput::new(&cli, &matches, &env, outcome.into_layers());
        let merge_ok =
            netsuke::cli::merge_with_cached_file_layers_with_observer(input, &mut observer).is_ok();
        (captured.snapshot(), merge_ok)
    });
    ensure!(!merge_ok, "file-sourced out-of-range jobs must be rejected");
    assert_contains(&events, "path_hash=")?;
    ensure!(
        events.iter().all(|event| !event.contains(&config_arg)),
        "configuration tracing must not record the raw config path: {events:#?}"
    );
    assert_contains(&events, "key=\"jobs\"")?;
    assert_contains(
        &events,
        "reason=\"job count is outside the supported range\"",
    )?;
    Ok(())
}

#[rstest]
fn merge_logs_no_input_validation_with_key_and_reason() -> Result<()> {
    let temp_dir = tempfile::tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    test_support::fs::write(&config_path, "no_input = false\n").context("write netsuke.toml")?;
    let config_arg = config_path.to_string_lossy().into_owned();
    let (events, merge_ok) =
        merge_and_capture(&["netsuke", "--config", &config_arg], &TestEnv::default())?;

    ensure!(!merge_ok, "no_input = false must be rejected");
    assert_contains(&events, "key=\"no_input\"")?;
    assert_contains(
        &events,
        "reason=\"no_input = false is unsupported because Netsuke has no interactive mode\"",
    )?;
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn merge_logs_malformed_environment_layer_failure() -> Result<()> {
    use std::os::unix::ffi::OsStringExt;

    let env = TestEnv {
        entries: vec![(
            OsString::from_vec(vec![b'N', b'E', b'T', b'S', b'U', b'K', b'E', b'_', 0xff]),
            OsString::from("value"),
        )],
    };
    let (events, merge_ok) = merge_and_capture(&["netsuke"], &env)?;

    ensure!(
        !merge_ok,
        "malformed Netsuke environment input must fail the merge"
    );
    assert_contains(&events, "layer=\"environment\"")?;
    assert_contains(&events, "environment configuration layer failed")?;
    Ok(())
}
