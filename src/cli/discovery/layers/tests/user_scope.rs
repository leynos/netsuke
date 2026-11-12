//! User-scope candidate derivation tests.
//!
//! These pin the Netsuke-owned user pass that must consider every user
//! location before the dependency's system scope. Each case supplies a valid
//! system file so a regression that restores the dependency's flat order is
//! observable as the system layer winning.

use super::super::super::discover_file_layers;
use super::super::super::paths::{FsPathNormalizer, normalized_path_key};
use super::{TestEnv, utf8_directory};
use crate::cli::Cli;
use anyhow::{Context, Result, ensure};
use pretty_assertions::assert_eq;
use std::path::Path;
use tempfile::tempdir;

/// Write a system-scope config file beneath `system`.
fn write_system_config(system: &Path) -> Result<()> {
    let config_dir = system.join("netsuke");
    test_support::fs::create_dir_all(&config_dir)
        .with_context(|| format!("create system config directory {}", config_dir.display()))?;
    test_support::fs::write(config_dir.join("config.toml"), "jobs = 9\n")
        .context("write system config")
}

/// Return the canonical path of the single discovered layer.
fn single_layer_path(discovered: &super::super::super::DiscoveryOutcome) -> Result<String> {
    let layers = discovered.layers();
    ensure!(
        layers.len() == 1,
        "expected exactly one discovered layer, got {layers:?}"
    );
    layers[0]
        .path()
        .map(|path| path.as_str().to_owned())
        .context("discovered layer should record its path")
}

/// Return the canonical form of `path` for comparison with a layer path.
fn canonical(path: &Path) -> Result<String> {
    Ok(
        normalized_path_key(&FsPathNormalizer, &path.to_string_lossy())
            .with_context(|| format!("canonicalise {}", path.display()))?
            .to_string_lossy()
            .into_owned(),
    )
}

/// Build a CLI rooted at an isolated project directory beneath `root`.
fn project_cli(root: &Path) -> Result<Cli> {
    let project = root.join("project");
    test_support::fs::create_dir(&project).context("create project directory")?;
    Ok(Cli {
        directory: Some(utf8_directory(&project)?),
        ..Cli::default()
    })
}

/// `USERPROFILE` supplies the home directory when `HOME` is unset, and that
/// user file must outrank a valid system file.
#[test]
fn userprofile_home_fallback_outranks_system_scope() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let cli = project_cli(temp.path())?;
    let home = temp.path().join("home");
    test_support::fs::create_dir(&home).context("create home directory")?;
    let user_config = home.join(".netsuke.toml");
    test_support::fs::write(&user_config, "jobs = 4\n").context("write user config")?;
    let system = temp.path().join("system");
    write_system_config(&system)?;

    let env = TestEnv::default()
        .with_var("USERPROFILE", home.as_os_str())
        .with_var("XDG_CONFIG_DIRS", system.as_os_str());
    let discovered = discover_file_layers(&cli, &env);

    ensure!(
        discovered.first_error().is_none(),
        "injected discovery should succeed"
    );
    assert_eq!(single_layer_path(&discovered)?, canonical(&user_config)?);
    Ok(())
}

/// An empty `XDG_CONFIG_HOME` is treated as unset, so the home user file must
/// still outrank a valid system file.
#[test]
fn empty_xdg_config_home_falls_back_to_home_user_file() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let cli = project_cli(temp.path())?;
    let home = temp.path().join("home");
    test_support::fs::create_dir(&home).context("create home directory")?;
    let user_config = home.join(".netsuke.toml");
    test_support::fs::write(&user_config, "jobs = 4\n").context("write user config")?;
    let system = temp.path().join("system");
    write_system_config(&system)?;

    let env = TestEnv::default()
        .with_var("HOME", home.as_os_str())
        .with_var("XDG_CONFIG_HOME", "")
        .with_var("XDG_CONFIG_DIRS", system.as_os_str());
    let discovered = discover_file_layers(&cli, &env);

    ensure!(
        discovered.first_error().is_none(),
        "injected discovery should succeed"
    );
    assert_eq!(single_layer_path(&discovered)?, canonical(&user_config)?);
    Ok(())
}

/// `HOME` outranks `USERPROFILE` when both name a user configuration file.
#[test]
fn home_outranks_userprofile_for_user_candidates() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let cli = project_cli(temp.path())?;
    let home = temp.path().join("home");
    let profile = temp.path().join("profile");
    test_support::fs::create_dir(&home).context("create home directory")?;
    test_support::fs::create_dir(&profile).context("create profile directory")?;
    let home_config = home.join(".netsuke.toml");
    test_support::fs::write(&home_config, "jobs = 4\n").context("write home config")?;
    test_support::fs::write(profile.join(".netsuke.toml"), "jobs = 7\n")
        .context("write profile config")?;
    let system = temp.path().join("system");
    write_system_config(&system)?;

    let env = TestEnv::default()
        .with_var("HOME", home.as_os_str())
        .with_var("USERPROFILE", profile.as_os_str())
        .with_var("XDG_CONFIG_DIRS", system.as_os_str());
    let discovered = discover_file_layers(&cli, &env);

    ensure!(
        discovered.first_error().is_none(),
        "injected discovery should succeed"
    );
    assert_eq!(single_layer_path(&discovered)?, canonical(&home_config)?);
    Ok(())
}
