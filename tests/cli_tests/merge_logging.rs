//! Structured debug-logging tests for the configuration merge pipeline.
//!
//! Verifies that `merge_with_config` emits a debug event at each layer
//! boundary (defaults, file discovery, environment, CLI overrides) and that
//! validation rejections carry structured `key`/`reason` fields.

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use std::sync::Arc;
use test_support::tracing_capture::with_test_subscriber;

fn merge_and_capture(cli_args: &[&str]) -> Result<(Vec<String>, bool)> {
    let _env_lock = test_support::env_lock::EnvLock::acquire();
    let localizer = Arc::from(netsuke::cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(cli_args, &localizer)
        .context("parse CLI args for merge logging test")?;
    Ok(with_test_subscriber(|captured| {
        let merge_ok = netsuke::cli::merge_with_config(&cli, &matches).is_ok();
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
    let _env_lock = test_support::env_lock::EnvLock::acquire();
    let temp_dir = tempfile::tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    std::fs::write(&config_path, "output_format = \"json\"\n").context("write netsuke.toml")?;
    let _config_guard =
        test_support::EnvVarGuard::set("NETSUKE_CONFIG_PATH", config_path.as_os_str());

    let localizer = Arc::from(netsuke::cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer)
        .context("parse CLI args")?;
    let (events, merge_ok) = with_test_subscriber(|captured| {
        let merge_ok = netsuke::cli::merge_with_config(&cli, &matches).is_ok();
        (captured.snapshot(), merge_ok)
    });
    ensure!(
        !merge_ok,
        "file-sourced output_format=json must be rejected"
    );
    assert_contains(&events, "key=\"output_format\"")?;
    assert_contains(&events, "reason=")?;
    Ok(())
}
