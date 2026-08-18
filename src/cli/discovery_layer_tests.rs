//! Tests for configuration file-layer collection.
//!
//! These cover which branch the shared file-layer boundary takes — explicit path
//! versus automatic discovery — and the project-scope second pass. Selector
//! precedence and event-schema snapshots live in the tracing test module.
use anyhow::{Context, Result, ensure};
use crate::cli::test_support::TestEnv;
use googletest::prelude::*;
use pretty_assertions::assert_eq;
use rstest::rstest;
use super::*;
use super::paths::{FailingPathNormalizer, FsPathNormalizer, normalized_path_key};
use tempfile::{TempDir, tempdir};

use std::cell::Cell;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use super::event_assertions::{EventAssertion, capture_events, find_event};
use super::layers::collect_file_layers_with_normalizer;
use super::paths::{FailingPathNormalizer, PathNormalizer};

#[derive(Debug, Clone, Copy)]
pub(super) enum LayerScenario {
    ExplicitConfig,
    Discovery,
}

/// Environment double that records discovery lookups for replay assertions.
#[derive(Default)]
pub(super) struct CountingEnv {
    get_calls: Cell<usize>,
}

impl CountingEnv {
    /// Return the number of selector reads performed so far.
    pub(super) fn get_calls(&self) -> usize {
        self.get_calls.get()
    }
}

impl EnvProvider for CountingEnv {
    fn get(&self, _key: &str) -> Option<OsString> {
        self.get_calls.set(self.get_calls.get() + 1);
        None
    }

    fn entries(&self) -> Vec<(OsString, OsString)> {
        Vec::new()
    }
}

/// Path normalizer that records project-key normalization calls.
#[derive(Default)]
struct CountingPathNormalizer {
    calls: Cell<usize>,
}

impl CountingPathNormalizer {
    /// Return the number of normalization calls performed so far.
    fn calls(&self) -> usize {
        self.calls.get()
    }
}

impl PathNormalizer for CountingPathNormalizer {
    fn normalize(&self, path: &Path) -> std::io::Result<PathBuf> {
        self.calls.set(self.calls.get() + 1);
        Ok(path.to_path_buf())
    }
}
/// Build a [`Cli`] for `scenario`, rooted in the isolated `temp` directory.
///
/// The explicit case writes a config file and selects it through `--config`; the
/// discovery case only anchors the project root, so discovery finds nothing.
pub(super) fn scenario_cli(scenario: LayerScenario, temp: &TempDir) -> Result<Cli> {
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

pub(super) fn replay_events(discovered: &DiscoveryOutcome) -> Result<Vec<String>> {
    let ((), events) = capture_events(|| {
        discovered.emit_diagnostics();
        Ok::<_, anyhow::Error>(())
    })?;
    Ok(events)
}

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

/// Cached automatic discovery replays its appended project-scope decision.
#[test]
fn replay_logs_discovery_and_appended_project_scope_without_environment_access() -> Result<()> {
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
    find_event(&events, "appending project-scope layers")?;

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
    let discovered = discover_file_layers(&Cli::default(), &env);
    ensure!(
        discovered.first_error().is_none(),
        "injected configuration discovery should succeed"
    );
    let layers = discovered.layers();
    let paths = layers
        .iter()
        .filter_map(|layer| layer.path().map(|path| path.as_str().to_owned()))
        .collect::<Vec<_>>();

    let expected_path = normalized_path_key(&FsPathNormalizer, &config_path.to_string_lossy())
        .context("canonicalise injected XDG config path")?
        .to_string_lossy()
        .into_owned();

    assert_eq!(paths, vec![expected_path]);
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
    let discovered = discover_file_layers(&cli, &env);

    if let Some(fragment) = expected_error_fragment {
        let error = discovered
            .first_error()
            .context("invalid discovered config must fail")?;
        assert_that!(error.to_string(), contains_substring(fragment));
    } else {
        ensure!(
            discovered.first_error().is_none(),
            "valid discovered config must load"
        );
        let layers = discovered.layers();
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
    let cli = Cli {
        directory: Some(non_canonical),
        ..Cli::default()
    };
    let discovered = discover_file_layers(&cli, &TestEnv::default());
    let layers = discovered.layers();

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
    let events = replay_events(&discovered)?;
    find_event(&events, "discovery included project-scope layers")?;
    Ok(())
}

/// A project-scope layer is not appended twice when the `--directory` alias
/// resolves to the same physical file through a different spelling.
///
/// On Windows the same file can be reached through a short-name form
/// (`C:\Users\RUNNER~1\...`) and a long-name form (`C:\Users\runneradmin\...`),
/// and `ortho_config` records the long-name canonical form. A symlink alias on
/// Unix exercises the same shape: the layer path recorded by discovery and the
/// key derived from the alias both canonicalise to the same physical file, so
/// the project-scope pass must not append the layer twice.
#[cfg(unix)]
#[test]
fn project_scope_layer_is_not_appended_twice_via_symlink_alias() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let project_dir = temp.path().join("project");
    test_support::fs::create_dir(&project_dir).context("create project dir")?;
    test_support::fs::write(
        project_dir.join(".netsuke.toml"),
        "default_targets = [\"alpha\"]\n",
    )
    .context("write project config")?;

    // An alternate spelling of `project_dir` that resolves to the same file.
    let alias = temp.path().join("project-alias");
    test_support::fs::symlink(&project_dir, &alias).context("create project alias")?;

    let (layers, events) = capture_events(|| {
        collect_file_layers_with_normalizer(Some(alias.as_path()), &paths::FsPathNormalizer)
    })?;

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

/// Scanning stored canonical layer paths does not normalize each inherited layer.
#[test]
fn project_layer_scan_normalizes_only_the_project_key() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let project_dir = temp.path().join("project");
    test_support::fs::create_dir(&project_dir).context("create project dir")?;
    test_support::fs::write(project_dir.join("base.toml"), "jobs = 7\n")
        .context("write inherited config")?;
    test_support::fs::write(
        project_dir.join(".netsuke.toml"),
        "extends = \"base.toml\"\njson = true\n",
    )
    .context("write project config")?;
    let normalizer = CountingPathNormalizer::default();

    let layers = collect_file_layers_with_normalizer(Some(project_dir.as_path()), &normalizer)
        .context("collect inherited project layers")?;

    ensure!(
        layers.len() == 2,
        "expected inherited and project layers: {layers:?}"
    );
    ensure!(
        normalizer.calls() == 1,
        "only the expected project path should be normalized, got {} calls",
        normalizer.calls()
    );
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

    let cli = Cli {
        directory: Some(project_dir),
        ..Cli::default()
    };
    let discovered = discover_file_layers(&cli, &TestEnv::default());
    let layers = discovered.layers();
    ensure!(layers.len() == 1, "expected one project layer: {layers:?}");
    Ok(())
}
