//! Deferred-diagnostic replay tests for configuration file-layer discovery.
//!
//! These tests share discovery fixtures with `layer_tests` and verify the
//! composition boundary replays its retained, bounded events without another
//! environment lookup.

use super::event_assertions::{EventAssertion, find_event};
use super::layer_tests::{CountingEnv, LayerScenario, replay_events, scenario_cli};
use super::*;
use anyhow::{Context, Result, ensure};
use tempfile::tempdir;

/// Cached explicit selection emits its branch without repeating discovery.
#[test]
fn replay_logs_explicit_config_branch_without_environment_access() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let cli = scenario_cli(LayerScenario::ExplicitConfig, &temp)?;
    let env = CountingEnv::default();
    let discovered = collect_diag_file_layers_with_env(&cli, &env);
    let events = replay_events(&discovered)?;
    find_event(&events, "resolved config path")?;
    let branch_event = find_event(&events, "using explicit config path")?;

    ensure!(
        !discovered.layers().is_empty(),
        "explicit layer should be retained for the merge"
    );
    ensure!(
        env.get_calls() == 0,
        "explicit selection should not access the environment, including on replay"
    );
    EventAssertion::new(
        branch_event,
        cli.config.as_deref().context("explicit config")?,
    )
    .ensure_bounded_path_fields()?;

    Ok(())
}

/// Cached automatic discovery emits no project-scope trace without a project layer.
#[test]
fn replay_logs_discovery_without_project_scope_trace_without_environment_access() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let cli = scenario_cli(LayerScenario::Discovery, &temp)?;
    let env = CountingEnv::default();
    let discovered = collect_diag_file_layers_with_env(&cli, &env);
    let discovery_get_calls = env.get_calls();
    ensure!(
        discovery_get_calls > 0,
        "discovery should read the injected environment"
    );

    let events = replay_events(&discovered)?;
    ensure!(
        env.get_calls() == discovery_get_calls,
        "replay must not access the environment again"
    );
    find_event(&events, "read config path variable")?;
    find_event(&events, "resolved config path")?;
    find_event(&events, "using config discovery")?;
    ensure!(
        !events
            .iter()
            .any(|event| event.contains("project-scope layers")),
        "discovery without a project layer must not report a project-scope outcome: {events:?}"
    );

    Ok(())
}

/// Cached automatic discovery replays its included project-scope decision.
#[test]
fn replay_logs_included_project_scope_without_environment_access() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    test_support::fs::write(temp.path().join(".netsuke.toml"), "jobs = 7\n")
        .context("write project config")?;
    let cli = scenario_cli(LayerScenario::Discovery, &temp)?;
    let env = CountingEnv::default();
    let discovered = collect_diag_file_layers_with_env(&cli, &env);
    let discovery_get_calls = env.get_calls();
    ensure!(
        discovery_get_calls > 0,
        "discovery should read the injected environment"
    );

    let events = replay_events(&discovered)?;
    ensure!(
        env.get_calls() == discovery_get_calls,
        "replay must not access the environment again"
    );
    find_event(&events, "using config discovery")?;
    find_event(&events, "discovery included project-scope layers")?;

    Ok(())
}
