//! Tests for configuration discovery tracing.

use super::*;
use anyhow::{Context, Result, ensure};
use rstest::{fixture, rstest};
use tempfile::{TempDir, tempdir};
use test_support::{EnvVarGuard, env_lock::EnvLock, tracing_capture::with_test_subscriber};
use tracing_subscriber::filter::LevelFilter;

fn capture_events<T, E>(
    test: impl FnOnce() -> std::result::Result<T, E>,
) -> std::result::Result<(T, Vec<String>), E> {
    with_test_subscriber(LevelFilter::TRACE, |captured| {
        let value = test()?;
        Ok((value, captured.snapshot()))
    })
}

fn find_event<'a>(events: &'a [String], message: &str) -> Result<&'a String> {
    events
        .iter()
        .find(|event| event.contains(message))
        .with_context(|| format!("expected event containing {message:?} in {events:?}"))
}

#[fixture]
fn clean_config_env() -> (EnvLock, EnvVarGuard, EnvVarGuard) {
    (
        EnvLock::acquire(),
        EnvVarGuard::remove(CONFIG_ENV_VAR),
        EnvVarGuard::remove(CONFIG_ENV_VAR_LEGACY),
    )
}

#[derive(Debug, Clone, Copy)]
enum EnvSetting {
    Removed,
    Set(&'static str),
}

#[derive(Debug, Clone, Copy)]
struct ConfigPathScenario {
    cli_config: Option<&'static str>,
    config_env: EnvSetting,
    legacy_env: EnvSetting,
    expected_path: Option<&'static str>,
    expected_selector: &'static str,
    expected_env_trace: Option<&'static str>,
}

#[derive(Debug, Clone, Copy)]
enum LayerScenario {
    ExplicitConfig,
    Discovery,
}

fn apply_env_setting(var_name: &'static str, setting: EnvSetting) -> Option<EnvVarGuard> {
    match setting {
        EnvSetting::Removed => None,
        EnvSetting::Set(value) => Some(EnvVarGuard::set(var_name, value)),
    }
}

#[rstest]
#[case::cli_flag_wins_over_environment(ConfigPathScenario {
    cli_config: Some("selected.toml"),
    config_env: EnvSetting::Removed,
    legacy_env: EnvSetting::Removed,
    expected_path: Some("selected.toml"),
    expected_selector: "cli_flag",
    expected_env_trace: None,
})]
#[case::primary_environment_wins_over_legacy(ConfigPathScenario {
    cli_config: None,
    config_env: EnvSetting::Set("env.toml"),
    legacy_env: EnvSetting::Set("legacy.toml"),
    expected_path: Some("env.toml"),
    expected_selector: CONFIG_ENV_VAR,
    expected_env_trace: Some(CONFIG_ENV_VAR),
})]
#[case::legacy_environment_used_when_primary_missing(ConfigPathScenario {
    cli_config: None,
    config_env: EnvSetting::Removed,
    legacy_env: EnvSetting::Set("legacy.toml"),
    expected_path: Some("legacy.toml"),
    expected_selector: CONFIG_ENV_VAR_LEGACY,
    expected_env_trace: Some(CONFIG_ENV_VAR_LEGACY),
})]
#[case::empty_environment_values_are_ignored(ConfigPathScenario {
    cli_config: None,
    config_env: EnvSetting::Set(""),
    legacy_env: EnvSetting::Set(""),
    expected_path: None,
    expected_selector: "none",
    expected_env_trace: None,
})]
#[case::missing_selectors_resolve_none(ConfigPathScenario {
    cli_config: None,
    config_env: EnvSetting::Removed,
    legacy_env: EnvSetting::Removed,
    expected_path: None,
    expected_selector: "none",
    expected_env_trace: None,
})]
fn explicit_config_path_logs_selected_selector(
    clean_config_env: (EnvLock, EnvVarGuard, EnvVarGuard),
    #[case] scenario: ConfigPathScenario,
) -> Result<()> {
    let _clean_config_env = clean_config_env;
    let _config_guard = apply_env_setting(CONFIG_ENV_VAR, scenario.config_env);
    let _legacy_guard = apply_env_setting(CONFIG_ENV_VAR_LEGACY, scenario.legacy_env);
    let cli = Cli {
        config: scenario.cli_config.map(PathBuf::from),
        ..Cli::default()
    };

    let (resolved, events) = capture_events(|| Ok::<_, anyhow::Error>(explicit_config_path(&cli)))?;
    let selector_event = find_event(&events, "resolved config path")?;

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
        Some(path) => ensure_bounded_path_fields(selector_event, path)?,
        None => ensure!(
            !selector_event.contains("path_hash=")
                && selector_event.contains("path_file_name=None"),
            "empty selection should not include path details: {selector_event}"
        ),
    }

    if let Some(var_name) = scenario.expected_env_trace {
        let env_event = events
            .iter()
            .find(|event| {
                event.contains("read config path variable")
                    && event.contains(&format!("var_name={var_name:?}"))
            })
            .with_context(|| format!("expected {var_name} trace event in {events:?}"))?;
        ensure!(
            env_event.contains("found=true"),
            "env trace should record that a path was found: {env_event}"
        );
    }

    Ok(())
}

#[rstest]
#[case::explicit_config_path(LayerScenario::ExplicitConfig, false, "using explicit config path")]
#[case::isolated_directory_discovery(LayerScenario::Discovery, true, "using config discovery")]
fn collect_diag_file_layers_logs_selected_branch(
    clean_config_env: (EnvLock, EnvVarGuard, EnvVarGuard),
    #[case] scenario: LayerScenario,
    #[case] should_be_empty: bool,
    #[case] expected_event: &str,
) -> Result<()> {
    let _clean_config_env = clean_config_env;
    let temp = tempdir().context("create temp dir")?;
    let cli = scenario_cli(scenario, &temp)?;

    let (layers, events) = capture_events(|| collect_diag_file_layers(&cli))?;
    let branch_event = find_event(&events, expected_event)?;

    ensure!(
        layers.is_empty() == should_be_empty,
        "layer collection result should match {scenario:?}"
    );
    ensure!(
        branch_event.contains(&format!("message={expected_event:?}"))
            || branch_event.contains(&format!("message={expected_event}")),
        "branch should emit the expected event: {branch_event}"
    );
    if matches!(scenario, LayerScenario::ExplicitConfig) {
        ensure_bounded_path_fields(
            branch_event,
            cli.config.as_deref().context("explicit config")?,
        )?;
        ensure!(
            !branch_event.contains("path="),
            "explicit path branch should avoid raw path fields: {branch_event}"
        );
    }

    Ok(())
}

#[test]
fn load_layers_from_path_logs_bounded_failure_fields() -> Result<()> {
    let missing_path = PathBuf::from("missing-secret-name.toml");

    let (error, events) = capture_events(|| {
        Ok::<_, anyhow::Error>(
            load_layers_from_path(&missing_path)
                .expect_err("missing explicit config file should fail"),
        )
    })?;
    let warn_event = find_event(&events, "explicit config load failed")?;
    let path_hash = short_hash(missing_path.to_string_lossy().as_bytes());

    ensure!(
        error.to_string().contains("missing-secret-name.toml"),
        "returned error should retain the diagnostic path"
    );
    ensure!(
        warn_event.contains("failure_kind=Missing"),
        "warn event should include bounded failure kind: {warn_event}"
    );
    ensure!(
        warn_event.contains(&format!("path_hash={path_hash}")),
        "warn event should include path hash: {warn_event}"
    );
    ensure!(
        warn_event.contains("path_file_name=Some(\"missing-secret-name.toml\")"),
        "warn event should include only the file name for path correlation: {warn_event}"
    );
    ensure!(
        !warn_event.contains("error="),
        "warn event should not include full formatted error text: {warn_event}"
    );
    Ok(())
}

fn ensure_bounded_path_fields(event: &str, path: &Path) -> Result<()> {
    let path_hash = short_hash(path.to_string_lossy().as_bytes());
    let file_name = path
        .file_name()
        .with_context(|| format!("expected file name for {}", path.display()))?;
    ensure!(
        event.contains(&format!("path_hash=\"{path_hash}\""))
            || event.contains(&format!("path_hash=Some(\"{path_hash}\")"))
            || event.contains(&format!("path_hash={path_hash}")),
        "event should include path hash for {}: {event}",
        path.display()
    );
    ensure!(
        event.contains(&format!("path_file_name=Some({file_name:?})")),
        "event should include path file name for {}: {event}",
        path.display()
    );
    Ok(())
}

fn scenario_cli(scenario: LayerScenario, temp: &TempDir) -> Result<Cli> {
    match scenario {
        LayerScenario::ExplicitConfig => {
            let config_path = temp.path().join("config.toml");
            std::fs::write(&config_path, "theme = \"ascii\"\n")
                .with_context(|| format!("write {}", config_path.display()))?;
            Ok(Cli {
                config: Some(config_path),
                ..Cli::default()
            })
        }
        LayerScenario::Discovery => Ok(Cli {
            directory: Some(temp.path().to_path_buf()),
            ..Cli::default()
        }),
    }
}
