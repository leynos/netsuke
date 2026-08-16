//! Tests for configuration file-layer collection.
//!
//! These cover which branch the shared file-layer boundary takes — explicit path
//! versus automatic discovery — and the project-scope second pass in
//! [`collect_file_layers`]. Selector precedence and event-schema snapshots live
//! in the tracing test module.

use super::*;
use crate::cli::test_support::TestEnv;
use anyhow::{Context, Result, ensure};
use googletest::prelude::*;
use pretty_assertions::assert_eq;
use rstest::rstest;
use tempfile::{TempDir, tempdir};

use super::event_assertions::{EventAssertion, capture_events, find_event};
use super::layers::{collect_file_layers, collect_file_layers_with_normalizer};
use super::paths::FailingPathNormalizer;

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

/// Automatic discovery must use the injected XDG directory, not the host.
#[test]
fn injected_automatic_discovery_uses_xdg_config_home() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let xdg_config_home = temp.path().join("xdg-config");
    let config_path = xdg_config_home.join("netsuke/config.toml");
    test_support::fs::create_dir(&xdg_config_home).context("create injected XDG directory")?;
    test_support::fs::create_dir(config_path.parent().context("config parent")?)
        .context("create injected config directory")?;
    test_support::fs::write(&config_path, "json = true\n").context("write injected config")?;

    let env = TestEnv::default().with_var("XDG_CONFIG_HOME", xdg_config_home.as_os_str());
    let sources = DiscoverySources::new(&env, discovery_env_source(&env));
    let layers = collect_file_layers_with_env(&Cli::default(), &sources)?;
    let paths = layers
        .iter()
        .filter_map(|layer| layer.path().map(|path| path.as_str().to_owned()))
        .collect::<Vec<_>>();

    assert_eq!(paths, vec![config_path.to_string_lossy().into_owned()]);
    Ok(())
}

/// Discovered configuration candidates retain the outcome that their content
/// warrants; an unreadable candidate is never mistaken for an absent one.
#[rstest]
#[case::no_candidate(None, None, 0)]
#[case::valid_candidate(Some("emoji = \"always\"\n"), None, 1)]
#[case::malformed_candidate(Some("emoji = \"always\n"), Some(".netsuke.toml"), 0)]
#[case::missing_parent(
    Some("extends = \"missing-parent.toml\"\n"),
    Some("missing-parent.toml"),
    0
)]
fn discovered_project_config_retains_load_outcome(
    #[case] contents: Option<&str>,
    #[case] expected_error_fragment: Option<&str>,
    #[case] expected_layer_count: usize,
) -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    if let Some(config_contents) = contents {
        test_support::fs::write(temp.path().join(".netsuke.toml"), config_contents)
            .context("write project config")?;
    }

    let cli = Cli {
        directory: Some(temp.path().to_path_buf()),
        ..Cli::default()
    };
    let env = TestEnv::default();
    let sources = DiscoverySources::new(&env, discovery_env_source(&env));
    let result = collect_file_layers_with_env(&cli, &sources);

    if let Some(fragment) = expected_error_fragment {
        let error = result.expect_err("invalid discovered config must fail");
        assert_that!(error.to_string(), contains_substring(fragment));
    } else {
        let layers = result.context("valid discovered config must load")?;
        assert_eq!(layers.len(), expected_layer_count);
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
    test_support::fs::create_dir(&project_dir).context("create project dir")?;
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

/// Normalization failure must not fail configuration discovery.
///
/// A missing project `.netsuke.toml` or an unreadable directory makes
/// canonicalization fail, which is ordinary rather than exceptional. The
/// discovery-side policy compares such a path literally and carries on; only an
/// unmatched project layer results, never an error.
#[test]
fn normalization_failure_does_not_fail_discovery() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let project_dir = temp.path().join("project");
    test_support::fs::create_dir(&project_dir).context("create project dir")?;
    test_support::fs::write(
        project_dir.join(".netsuke.toml"),
        "default_targets = [\"alpha\"]\n",
    )
    .context("write project config")?;

    let layers =
        collect_file_layers_with_normalizer(Some(project_dir.as_path()), &FailingPathNormalizer)
            .context("discovery must succeed despite normalization failure")?;

    ensure!(layers.len() == 1, "expected one project layer: {layers:?}");
    Ok(())
}

/// A non-UTF-8 project directory still matches the discovered project layer.
#[cfg(unix)]
#[test]
fn non_utf8_directory_does_not_duplicate_project_layer() -> Result<()> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let temp = tempdir().context("create temp dir")?;
    let project_dir = temp
        .path()
        .join(OsString::from_vec(b"project-\xff".to_vec()));
    test_support::fs::create_dir(&project_dir).context("create project dir")?;
    // OrthoConfig records paths lossily, so provide the equivalent alias it
    // uses for filesystem access while retaining a non-UTF-8 `--directory`.
    let lossy_project_dir = temp.path().join("project-\u{fffd}");
    test_support::fs::symlink(&project_dir, &lossy_project_dir)
        .context("create lossy project alias")?;
    test_support::fs::write(
        project_dir.join(".netsuke.toml"),
        "default_targets = [\"alpha\"]\n",
    )
    .context("write project config")?;

    let layers = collect_file_layers(Some(project_dir.as_path()))?;
    ensure!(layers.len() == 1, "expected one project layer: {layers:?}");
    Ok(())
}
