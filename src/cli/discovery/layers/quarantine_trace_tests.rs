//! Tests for bounded environment-grant quarantine diagnostics.
//!
//! These replay discovery telemetry through the normal diagnostic boundary and
//! check that only project-controlled occurrences produce quarantine events.

use super::tests::{CountingEnv, LayerScenario, replay_events, scenario_cli};
use crate::cli::discovery::collect_diag_file_layers_with_env;
use crate::cli::test_support::TestEnv;
use anyhow::{Context, Result, ensure};
use tempfile::tempdir;

use crate::cli::discovery::event_assertions::find_event;

/// Replay one bounded quarantine event for an automatically discovered chain.
#[test]
fn replay_reports_bounded_project_chain_grant_quarantine() -> Result<()> {
    let temp = tempdir().context("create project directory")?;
    test_support::fs::write(
        temp.path().join(".netsuke.toml"),
        "extends = \"base.toml\"\n",
    )
    .context("write project config")?;
    test_support::fs::write(
        temp.path().join("base.toml"),
        "env_allow_var = [\"CI_TOKEN\"]\n",
    )
    .context("write project ancestor")?;
    let cli = scenario_cli(LayerScenario::Discovery, &temp)?;
    let env = CountingEnv::default();
    let discovered = collect_diag_file_layers_with_env(&cli, &env);
    let discovery_lookups = env.get_calls();

    let events = replay_events(&discovered)?;
    let event = find_event(&events, "quarantined project environment grants")?;
    ensure!(
        event.contains("authority=\"project_chain\"") && event.contains("grant_count=\"one\""),
        "the event should expose only bounded provenance and count: {event}"
    );
    ensure!(
        !event.contains("CI_TOKEN") && !event.contains("base.toml"),
        "the event must not expose grant names or configuration paths: {event}"
    );
    ensure!(
        env.get_calls() == discovery_lookups,
        "diagnostic replay must not repeat environment access"
    );
    Ok(())
}

/// Leave operator-rooted grants out of project quarantine telemetry.
#[test]
fn replay_omits_project_quarantine_event_for_operator_chain() -> Result<()> {
    let temp = tempdir().context("create isolated config roots")?;
    let xdg_home = temp.path().join("xdg");
    let config_dir = xdg_home.join("netsuke");
    test_support::fs::create_dir_all(&config_dir).context("create XDG config directory")?;
    test_support::fs::write(config_dir.join("config.toml"), "extends = \"base.toml\"\n")
        .context("write operator config")?;
    test_support::fs::write(
        config_dir.join("base.toml"),
        "env_allow_var = [\"CI_TOKEN\"]\n",
    )
    .context("write operator ancestor")?;
    let cli = scenario_cli(LayerScenario::Discovery, &temp)?;
    let env = TestEnv::default().with_var("XDG_CONFIG_HOME", xdg_home.as_os_str());
    let discovered = collect_diag_file_layers_with_env(&cli, &env);

    let events = replay_events(&discovered)?;
    ensure!(
        !events
            .iter()
            .any(|event| event.contains("quarantined project environment grants")),
        "operator grants must not be reported as project-quarantined: {events:?}"
    );
    Ok(())
}
