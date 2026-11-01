//! Differential tests for the restricted automatic discovery walk.
//!
//! OrthoConfig's `compose_layers()` remains the reference implementation for
//! ordinary configuration: the restricted walk exists only so each candidate
//! root can be classified before its chain is loaded. For a configuration with
//! no authority claim in it, both traversals must therefore observe the same
//! layers in the same order, or fail identically. These tests keep that
//! equivalence honest for missing, malformed, inherited, and successful files.

use super::super::super::paths::FsPathNormalizer;
use super::super::{ProjectKey, config_discovery, first_automatic_chain};
use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use ortho_config::{ConfigDiscovery, DiscoveryLayersOutcome, MapEnv, MergeLayer, SharedEnvSource};
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;
use tempfile::tempdir;

/// Environment that isolates discovery from the host's configuration.
///
/// Selector, home, and XDG variables all point at paths inside `root` that the
/// test never creates, so only the candidates the test writes can be found.
fn isolated_env(root: &Path) -> SharedEnvSource {
    isolated_env_with_xdg(root, &root.join("absent").join("home"))
}

/// Isolate discovery while placing the user configuration under `xdg_home`.
fn isolated_env_with_xdg(root: &Path, xdg_home: &Path) -> SharedEnvSource {
    let mut env = MapEnv::new();
    let absent = root.join("absent");
    env.insert("HOME", absent.clone().into_os_string());
    env.insert("XDG_CONFIG_HOME", xdg_home.as_os_str().to_owned());
    env.insert("XDG_CONFIG_DIRS", absent.join("dirs").into_os_string());
    Arc::new(env)
}

/// Collapse layers into the path and value pair a comparison can observe.
fn flatten(layers: Vec<MergeLayer<'static>>) -> Vec<(Option<Utf8PathBuf>, Value)> {
    layers
        .into_iter()
        .map(|layer| (layer.path().map(Utf8Path::to_owned), layer.into_value()))
        .collect()
}

/// Reproduce the pre-M1 outcome using OrthoConfig's own composition.
fn legacy_outcome(
    discovery: &ConfigDiscovery,
) -> Result<Vec<(Option<Utf8PathBuf>, Value)>, String> {
    let DiscoveryLayersOutcome {
        value,
        mut required_errors,
        mut optional_errors,
    } = discovery.compose_layers();
    if value.is_empty() {
        required_errors.append(&mut optional_errors);
    }
    match required_errors.into_iter().next() {
        Some(error) => Err(error.to_string()),
        None => Ok(flatten(value)),
    }
}

/// Produce the restricted walk's outcome in the same comparable shape.
fn restricted_outcome(
    discovery: &ConfigDiscovery,
    project_key: Option<&ProjectKey>,
) -> Result<Vec<(Option<Utf8PathBuf>, Value)>, String> {
    first_automatic_chain(discovery, project_key)
        .map(|(layers, _)| flatten(layers))
        .map_err(|error| error.to_string())
}

/// Build a discovery scanner anchored at `project_dir` for both walks.
fn scanner(project_dir: &Path, root: &Path) -> ConfigDiscovery {
    config_discovery(Some(&project_dir.to_path_buf()), isolated_env(root))
}

/// Compare the two traversals for the project file at `project_dir`.
fn compare_outcomes(
    project_dir: &Path,
    root: &Path,
) -> Result<(
    Vec<(Option<Utf8PathBuf>, Value)>,
    Vec<(Option<Utf8PathBuf>, Value)>,
)> {
    let discovery = scanner(project_dir, root);
    let project_key = ProjectKey::classify(&FsPathNormalizer, &project_dir.join(".netsuke.toml"));
    let legacy = legacy_outcome(&discovery);
    let restricted = restricted_outcome(&discovery, Some(&project_key));
    match (&legacy, &restricted) {
        (Ok(legacy_layers), Ok(restricted_layers)) => {
            ensure!(
                legacy_layers == restricted_layers,
                "restricted walk observed {restricted_layers:?}, ortho observed {legacy_layers:?}"
            );
            Ok((legacy_layers.clone(), restricted_layers.clone()))
        }
        (Err(legacy_error), Err(restricted_error)) => {
            ensure!(
                legacy_error == restricted_error,
                "failure text diverged: ortho said {legacy_error:?}, restricted said {restricted_error:?}"
            );
            Ok((Vec::new(), Vec::new()))
        }
        _ => anyhow::bail!("outcome shape diverged: legacy={legacy:?}, restricted={restricted:?}"),
    }
}

#[test]
fn missing_configuration_matches_ortho_discovery() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let project_dir = temp.path().join("project");
    test_support::fs::create_dir(&project_dir).context("create project dir")?;

    let (legacy, _) = compare_outcomes(&project_dir, temp.path())?;

    let project_path = Utf8PathBuf::from_path_buf(project_dir.join(".netsuke.toml"))
        .map_err(|path| anyhow::anyhow!("project path is not UTF-8: {}", path.display()))?;
    ensure!(
        legacy
            .iter()
            .all(|(path, _)| path.as_ref() != Some(&project_path)),
        "a missing project file must not yield a project layer"
    );
    Ok(())
}

#[test]
fn malformed_configuration_matches_ortho_discovery() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let project_dir = temp.path().join("project");
    test_support::fs::create_dir(&project_dir).context("create project dir")?;
    test_support::fs::write(project_dir.join(".netsuke.toml"), "jobs = [\n")
        .context("write malformed project config")?;

    let discovery = scanner(&project_dir, temp.path());
    let project_key = ProjectKey::classify(&FsPathNormalizer, &project_dir.join(".netsuke.toml"));

    let legacy = legacy_outcome(&discovery);
    let restricted = restricted_outcome(&discovery, Some(&project_key));

    ensure!(
        legacy.is_err() && restricted.is_err(),
        "a malformed configuration must fail in both traversals"
    );
    ensure!(
        legacy == restricted,
        "both traversals must report the same load failure"
    );
    Ok(())
}

#[test]
fn inherited_configuration_matches_ortho_discovery() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let project_dir = temp.path().join("project");
    test_support::fs::create_dir(&project_dir).context("create project dir")?;
    test_support::fs::write(project_dir.join("base.toml"), "jobs = 1\n")
        .context("write inherited config")?;
    test_support::fs::write(
        project_dir.join(".netsuke.toml"),
        "extends = \"base.toml\"\ndefault_targets = [\"alpha\"]\n",
    )
    .context("write project config")?;

    let (legacy, restricted) = compare_outcomes(&project_dir, temp.path())?;

    let names = |layers: &[(Option<Utf8PathBuf>, Value)]| -> Vec<String> {
        layers
            .iter()
            .filter_map(|(path, _)| path.as_deref())
            .filter_map(Utf8Path::file_name)
            .map(ToOwned::to_owned)
            .collect()
    };
    ensure!(
        names(&legacy) == ["base.toml", ".netsuke.toml"],
        "the inherited chain should keep both occurrences in dependency order"
    );
    ensure!(
        names(&restricted) == names(&legacy),
        "inherited occurrences must be ordered identically"
    );
    Ok(())
}

#[test]
fn successful_configuration_matches_ortho_discovery() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let project_dir = temp.path().join("project");
    let xdg_home = temp.path().join("xdg-home");
    test_support::fs::create_dir(&project_dir).context("create project dir")?;
    test_support::fs::create_dir_all(xdg_home.join("netsuke")).context("create xdg config dir")?;
    test_support::fs::write(xdg_home.join("netsuke/config.toml"), "jobs = 2\n")
        .context("write operator config")?;

    let discovery = config_discovery(
        Some(&project_dir),
        isolated_env_with_xdg(temp.path(), &xdg_home),
    );
    let project_key = ProjectKey::classify(&FsPathNormalizer, &project_dir.join(".netsuke.toml"));

    let legacy = legacy_outcome(&discovery);
    let restricted = restricted_outcome(&discovery, Some(&project_key));

    ensure!(
        legacy == restricted,
        "an ordinary operator configuration must resolve identically"
    );
    let layers = legacy.map_err(anyhow::Error::msg)?;
    let loaded = layers
        .first()
        .and_then(|(path, _)| path.as_deref())
        .context("the operator configuration should record its path")?;
    ensure!(
        loaded.ends_with("netsuke/config.toml") && layers.len() == 1,
        "the xdg candidate should supply the only layer, got {layers:?}"
    );
    Ok(())
}
