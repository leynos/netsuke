//! Structured debug-logging tests for the configuration merge pipeline.
//!
//! Verifies that `merge_with_config` emits a debug event at each layer
//! boundary (defaults, file discovery, environment, CLI overrides) and that
//! validation rejections carry structured `key`/`reason` fields.

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use std::{ffi::OsString, sync::Arc};
use test_support::tracing_capture::with_test_subscriber;
use tracing_subscriber::filter::LevelFilter;

#[derive(Default)]
struct TestEnv;

impl netsuke::cli::ConfigEnvProvider for TestEnv {
    fn get(&self, _key: &str) -> Option<OsString> {
        None
    }
}

fn merge_and_capture(cli_args: &[&str]) -> Result<(Vec<String>, bool)> {
    let localizer = Arc::from(netsuke::cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(cli_args, &localizer)
        .context("parse CLI args for merge logging test")?;
    Ok(with_test_subscriber(LevelFilter::DEBUG, |captured| {
        let merge_ok = netsuke::cli::merge_with_config_and_env(&cli, &matches, &TestEnv).is_ok();
        (captured.snapshot(), merge_ok)
    }))
}

fn assert_contains(events: &[String], needle: &str) -> Result<()> {
    ensure!(
        events.iter().any(|event| event.contains(needle)),
        "expected a captured event containing {needle:?}; got {events:#?}"
    );
    Ok(())
}

#[rstest]
fn merge_emits_debug_event_per_layer() -> Result<()> {
    let (events, merge_ok) = merge_and_capture(&["netsuke"])?;
    ensure!(merge_ok, "merge should succeed for plain invocation");
    assert_contains(&events, "layer=\"defaults\"")?;
    assert_contains(&events, "layer=\"file\"")?;
    assert_contains(&events, "layer=\"environment\"")?;
    assert_contains(&events, "layer=\"cli\"")?;
    Ok(())
}

#[rstest]
fn merge_logs_explicit_cli_override_keys() -> Result<()> {
    let (events, merge_ok) = merge_and_capture(&["netsuke", "--jobs", "3"])?;
    ensure!(merge_ok, "merge should succeed with --jobs override");
    assert_contains(&events, "override_keys")?;
    assert_contains(&events, "jobs")?;
    Ok(())
}

#[rstest]
fn merge_logs_validation_rejection_with_key_and_reason() -> Result<()> {
    let temp_dir = tempfile::tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    std::fs::write(&config_path, "jobs = 0\n").context("write netsuke.toml")?;

    let localizer = Arc::from(netsuke::cli_localization::build_localizer(None));
    let config_arg = config_path.to_string_lossy().into_owned();
    let (cli, matches) =
        netsuke::cli::parse_with_localizer_from(["netsuke", "--config", &config_arg], &localizer)
            .context("parse CLI args")?;
    let (events, merge_ok) = with_test_subscriber(LevelFilter::DEBUG, |captured| {
        let merge_ok = netsuke::cli::merge_with_config_and_env(&cli, &matches, &TestEnv).is_ok();
        (captured.snapshot(), merge_ok)
    });
    ensure!(!merge_ok, "file-sourced out-of-range jobs must be rejected");
    assert_contains(&events, "key=\"jobs\"")?;
    assert_contains(&events, "reason=")?;
    Ok(())
}
