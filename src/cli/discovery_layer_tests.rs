//! Tests for configuration file-layer collection.
//!
//! These cover which branch the shared file-layer boundary takes — explicit path
//! versus automatic discovery — and the project-scope second pass in
//! [`collect_file_layers`]. Selector precedence and event-schema snapshots live
//! in the tracing test module.

use super::*;
use crate::cli::test_support::TestEnv;
use anyhow::{Context, Result, ensure};
use rstest::rstest;
use tempfile::{TempDir, tempdir};
use test_support::tracing_capture::with_test_subscriber;
use tracing_subscriber::filter::LevelFilter;

use super::event_assertions::EventAssertion;

/// Run `test` under a TRACE-level capturing subscriber.
fn capture_events<T, E>(
    test: impl FnOnce() -> std::result::Result<T, E>,
) -> std::result::Result<(T, Vec<String>), E> {
    with_test_subscriber(LevelFilter::TRACE, |captured| {
        let value = test()?;
        Ok((value, captured.snapshot()))
    })
}

/// Return the first captured event containing `message`.
fn find_event<'a>(events: &'a [String], message: &str) -> Result<&'a String> {
    events
        .iter()
        .find(|event| event.contains(message))
        .with_context(|| format!("expected event containing {message:?} in {events:?}"))
}

#[derive(Debug, Clone, Copy)]
enum LayerScenario {
    ExplicitConfig,
    Discovery,
}

/// Build a [`Cli`] for `scenario`, rooted in the isolated `temp` directory.
///
/// The explicit case writes a config file and selects it through `--config`; the
/// discovery case only anchors the project root, so discovery finds nothing.
fn scenario_cli(scenario: LayerScenario, temp: &TempDir) -> Result<Cli> {
    match scenario {
        LayerScenario::ExplicitConfig => {
            let config_path = temp.path().join("config.toml");
            test_support::fs::write(&config_path, "theme = \"ascii\"\n")
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

#[rstest]
#[case::explicit_config_path(LayerScenario::ExplicitConfig, false, "using explicit config path")]
#[case::isolated_directory_discovery(LayerScenario::Discovery, true, "using config discovery")]
fn collect_diag_file_layers_logs_selected_branch(
    #[case] scenario: LayerScenario,
    #[case] should_be_empty: bool,
    #[case] expected_event: &str,
) -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let cli = scenario_cli(scenario, &temp)?;
    let env = TestEnv::default();

    let (layers, events) = capture_events(|| collect_diag_file_layers_with_env(&cli, &env))?;
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
        EventAssertion::new(
            branch_event,
            cli.config.as_deref().context("explicit config")?,
        )
        .ensure_bounded_path_fields()?;
        ensure!(
            !branch_event.contains("path="),
            "explicit path branch should avoid raw path fields: {branch_event}"
        );
    }

    Ok(())
}

/// A project-scope layer already found by discovery is not appended again.
///
/// `OrthoConfig` records canonicalised layer paths, so a non-canonical
/// `--directory` (here one containing a `.` component, as a relative or
/// symlinked path would be) must still match. Appending twice would duplicate
/// the entries of every `merge_strategy = "append"` field in the file.
#[test]
fn existing_project_scope_layer_is_not_appended_twice() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let project_dir = temp.path().join("project");
    std::fs::create_dir(&project_dir).context("create project dir")?;
    test_support::fs::write(
        project_dir.join(".netsuke.toml"),
        "default_targets = [\"alpha\"]\n",
    )
    .context("write project config")?;

    // Equivalent to `project_dir`, but not in canonical form.
    let non_canonical = project_dir.join(".");
    let (layers, events) = capture_events(|| collect_file_layers(Some(non_canonical.as_path())))?;

    let project_layers = layers
        .iter()
        .filter(|layer| {
            layer
                .path()
                .is_some_and(|path| path.as_str().ends_with(".netsuke.toml"))
        })
        .count();
    ensure!(
        project_layers == 1,
        "project-scope layer should appear exactly once, found {project_layers}: {layers:?}"
    );
    find_event(&events, "discovery included project-scope layers")?;
    Ok(())
}
