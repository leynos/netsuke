//! Tests for configuration discovery tracing.
//!
//! Selection is exercised through the injected [`EnvProvider`] double, so these
//! tests never mutate the process environment and need no lock.

use super::*;
use crate::cli::test_support::TestEnv;
use crate::snapshot_test_support::snapshot_settings;
use anyhow::{Context, Result, ensure};
use insta::assert_snapshot;
use rstest::rstest;
use tempfile::tempdir;

use super::event_assertions::{EventAssertion, capture_events, find_event};

/// Snapshot `assertion`'s event under `snapshot_name`, with its hash normalized.
///
/// The `insta` call lives here rather than on [`EventAssertion`] so snapshot
/// names stay bound to this module.
fn snapshot_failure_event(assertion: &EventAssertion<'_>, snapshot_name: &str) -> Result<()> {
    let normalized = assertion.normalize_path_hash()?;
    snapshot_settings("discovery").bind(|| {
        assert_snapshot!(snapshot_name, normalized);
    });
    Ok(())
}

/// Resolve `cli_config`/`env_config` and trace the result, returning both.
fn resolve_and_trace(
    cli_config: Option<PathBuf>,
    env: &TestEnv,
) -> Result<(ConfigPathResolution, Vec<String>)> {
    capture_events(|| {
        let resolution = resolve_config_selector(cli_config, env);
        trace_config_path_resolution(&resolution);
        Ok::<_, anyhow::Error>(resolution)
    })
}

#[derive(Debug, Clone, Copy)]
struct ConfigPathScenario {
    cli_config: Option<&'static str>,
    config_env: Option<&'static str>,
    expected_path: Option<&'static str>,
    expected_selector: &'static str,
    /// Expected environment lookup trace as `(var_name, found)`.
    ///
    /// `None` means no lookup is expected at all, which happens only when the
    /// CLI flag short-circuits resolution. Otherwise the lookup must be traced,
    /// with `found` distinguishing a resolved path from a missing or empty one.
    expected_env_trace: Option<(&'static str, bool)>,
}

/// The selector event names the winning selector and bounds its path fields.
///
/// Each case sets one presence combination of `--config` and `NETSUKE_CONFIG`
/// and asserts the resolved path, the `selector`/`path_present` fields, and the
/// per-variable lookup trace.
#[rstest]
#[case::cli_flag_wins_over_environment(ConfigPathScenario {
    cli_config: Some("selected.toml"),
    config_env: None,
    expected_path: Some("selected.toml"),
    expected_selector: "cli_flag",
    expected_env_trace: None,
})]
#[case::primary_environment_selected(ConfigPathScenario {
    cli_config: None,
    config_env: Some("env.toml"),
    expected_path: Some("env.toml"),
    expected_selector: CONFIG_ENV_VAR,
    expected_env_trace: Some((CONFIG_ENV_VAR, true)),
})]
#[case::empty_environment_values_are_ignored(ConfigPathScenario {
    cli_config: None,
    config_env: Some(""),
    expected_path: None,
    expected_selector: "none",
    expected_env_trace: Some((CONFIG_ENV_VAR, false)),
})]
#[case::missing_selectors_resolve_none(ConfigPathScenario {
    cli_config: None,
    config_env: None,
    expected_path: None,
    expected_selector: "none",
    expected_env_trace: Some((CONFIG_ENV_VAR, false)),
})]
fn explicit_config_path_logs_selected_selector(#[case] scenario: ConfigPathScenario) -> Result<()> {
    let mut env = TestEnv::default();
    if let Some(value) = scenario.config_env {
        env = env.with_var(CONFIG_ENV_VAR, value);
    }

    let (resolution, events) = resolve_and_trace(scenario.cli_config.map(PathBuf::from), &env)?;
    let selector_event = find_event(&events, "resolved config path")?;
    let resolved = resolution.path;

    ensure!(
        resolved == scenario.expected_path.map(PathBuf::from),
        "expected selected config path for {scenario:?}"
    );
    ensure!(
        selector_event.contains(&format!("selector={:?}", scenario.expected_selector)),
        "selector field should identify winner: {selector_event}"
    );
    ensure!(
        selector_event.contains(&format!("path_present={}", resolved.is_some())),
        "path_present should record whether a path was selected: {selector_event}"
    );
    match resolved.as_deref() {
        Some(path) => EventAssertion::new(selector_event, path).ensure_bounded_path_fields()?,
        // `Option<T>: Value::record` omits `None`, while `record_debug` renders
        // it as `path_file_name=None`; that asymmetry explains these checks.
        None => ensure!(
            !selector_event.contains("path_hash=")
                && selector_event.contains("path_file_name=None"),
            "empty selection should not include path details: {selector_event}"
        ),
    }

    match scenario.expected_env_trace {
        Some((var_name, found)) => {
            let env_event = events
                .iter()
                .find(|event| {
                    event.contains("read config path variable")
                        && event.contains(&format!("var_name={var_name:?}"))
                })
                .with_context(|| format!("expected {var_name} trace event in {events:?}"))?;
            ensure!(
                env_event.contains(&format!("found={found}")),
                "env trace should record found={found}: {env_event}"
            );
        }
        None => ensure!(
            !events
                .iter()
                .any(|event| event.contains("read config path variable")),
            "cli selection should not read any config path variable: {events:?}"
        ),
    }

    Ok(())
}

/// The removed `NETSUKE_CONFIG_PATH` alias must not select a config file.
///
/// ADR-004 keeps `NETSUKE_CONFIG` as the only environment selector, and #427
/// removed the legacy alias. Setting it alone therefore resolves to `none`, and
/// only `NETSUKE_CONFIG` is ever looked up.
#[test]
fn legacy_config_path_variable_is_not_a_selector() -> Result<()> {
    let env = TestEnv::default().with_var("NETSUKE_CONFIG_PATH", "legacy-should-be-ignored.toml");

    let (resolution, events) = resolve_and_trace(None, &env)?;

    ensure!(
        resolution.path.is_none(),
        "legacy variable must not select a config path: {resolution:?}"
    );
    ensure!(
        resolution.selector == "none",
        "selector should be none, got {:?}",
        resolution.selector
    );
    ensure!(
        !events
            .iter()
            .any(|event| event.contains("NETSUKE_CONFIG_PATH")),
        "the legacy variable should never be looked up: {events:?}"
    );
    Ok(())
}

#[test]
fn selector_resolution_event_schema_snapshot() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let config_path = temp.path().join("selector.toml");
    let env = TestEnv::default().with_var(CONFIG_ENV_VAR, config_path.as_os_str());

    let (resolution, events) = resolve_and_trace(None, &env)?;
    ensure!(
        resolution.path.as_deref() == Some(config_path.as_path()),
        "primary environment path should be selected"
    );

    let env_event = find_event(&events, "read config path variable")?;
    let selector_event = find_event(&events, "resolved config path")?;
    EventAssertion::new(env_event, &config_path).ensure_raw_path_absent()?;
    EventAssertion::new(selector_event, &config_path).ensure_raw_path_absent()?;
    let normalized = [env_event, selector_event]
        .map(|event| EventAssertion::new(event, &config_path).normalize_path_hash())
        .into_iter()
        .collect::<Result<Vec<_>>>()?
        .join("\n");

    snapshot_settings("discovery").bind(|| {
        assert_snapshot!("selector_resolution_event_schema", normalized);
    });
    Ok(())
}

#[test]
fn load_layers_from_path_logs_bounded_failure_fields() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let missing_path = temp.path().join("missing-secret-name.toml");

    let (error, events) = capture_events(|| {
        Ok::<_, anyhow::Error>(
            load_layers_from_path(&missing_path)
                .expect_err("missing explicit config file should fail"),
        )
    })?;
    let warn_event = find_event(&events, "explicit config load failed")?;
    let assertion = EventAssertion::new(warn_event, &missing_path);

    ensure!(
        error.to_string().contains("missing-secret-name.toml"),
        "returned error should retain the diagnostic path"
    );
    ensure!(
        warn_event.contains("failure_kind=Missing"),
        "warn event should include bounded failure kind: {warn_event}"
    );
    assertion.ensure_bounded_path_fields()?;
    ensure!(
        !warn_event.contains("error="),
        "warn event should not include full formatted error text: {warn_event}"
    );
    assertion.ensure_private_event_fields(&error.to_string())?;
    snapshot_failure_event(&assertion, "explicit_load_missing_event_schema")?;
    Ok(())
}

#[test]
fn load_layers_from_path_logs_invalid_toml_failure() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let config_path = temp.path().join("invalid-secret-config.toml");
    test_support::fs::write(&config_path, "theme = [invalid parser secret\n")
        .with_context(|| format!("write {}", config_path.display()))?;

    let (error, events) = capture_events(|| {
        Ok::<_, anyhow::Error>(
            load_layers_from_path(&config_path)
                .expect_err("invalid explicit config file should fail"),
        )
    })?;
    let warn_event = find_event(&events, "explicit config load failed")?;
    let formatted_error = error.to_string();

    ensure!(
        warn_event.contains("failure_kind=LoadError"),
        "warn event should classify parser failures: {warn_event}"
    );
    let assertion = EventAssertion::new(warn_event, &config_path);
    assertion.ensure_bounded_path_fields()?;
    assertion.ensure_private_event_fields(&formatted_error)?;
    ensure!(
        !warn_event.contains("invalid parser secret"),
        "warn event should not contain parser input: {warn_event}"
    );
    snapshot_failure_event(&assertion, "explicit_load_error_event_schema")?;
    Ok(())
}
