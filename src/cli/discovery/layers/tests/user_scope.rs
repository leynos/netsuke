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
use std::path::{Path, PathBuf};
use tempfile::{TempDir, tempdir};

/// Write a system-scope config file beneath `system`.
fn write_system_config(system: &Path) -> Result<()> {
    let config_dir = system.join("netsuke");
    test_support::fs::create_dir_all(&config_dir)
        .with_context(|| format!("create system config directory {}", config_dir.display()))?;
    test_support::fs::write(config_dir.join("config.toml"), "jobs = 9\n")
        .context("write system config")
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

/// Isolated project, home, and system roots for one user-scope case.
///
/// The system root always holds a valid configuration, so a case that passes
/// only because the system file went unread shows up as the wrong layer
/// winning.
struct Case {
    /// Keeps the scenario directories alive for the whole case.
    temp: TempDir,
    cli: Cli,
    home: PathBuf,
    system: PathBuf,
}

impl Case {
    /// Build a project CLI plus home and system roots.
    fn new() -> Result<Self> {
        let temp = tempdir().context("create temp dir")?;
        let cli = project_cli(temp.path())?;
        let home = temp.path().join("home");
        test_support::fs::create_dir(&home).context("create home directory")?;
        let system = temp.path().join("system");
        write_system_config(&system)?;
        Ok(Self {
            temp,
            cli,
            home,
            system,
        })
    }

    /// Return the scenario root that holds the home and system directories.
    fn root(&self) -> &Path {
        self.temp.path()
    }

    /// Route the system scope at this case's system root.
    fn with_system_scope(&self, env: TestEnv) -> TestEnv {
        env.with_var("XDG_CONFIG_DIRS", self.system.as_os_str())
    }

    /// Discover configuration from `env` and return the single layer path.
    fn discover(&self, env: &TestEnv) -> Result<String> {
        let discovered = discover_file_layers(&self.cli, env);
        ensure!(
            discovered.first_error().is_none(),
            "injected discovery should succeed"
        );
        let layers = discovered.layers();
        ensure!(
            layers.len() == 1,
            "expected exactly one discovered layer, got {layers:?}"
        );
        layers
            .first()
            .context("discovered layer should be present")?
            .path()
            .map(|path| path.as_str().to_owned())
            .context("discovered layer should record its path")
    }
}

/// `USERPROFILE` supplies the home directory when `HOME` is unset, and that
/// user file must outrank a valid system file.
#[test]
fn userprofile_home_fallback_outranks_system_scope() -> Result<()> {
    let case = Case::new()?;
    let user_config = case.home.join(".netsuke.toml");
    test_support::fs::write(&user_config, "jobs = 4\n").context("write user config")?;
    let env =
        case.with_system_scope(TestEnv::default().with_var("USERPROFILE", case.home.as_os_str()));

    assert_eq!(case.discover(&env)?, canonical(&user_config)?);
    Ok(())
}

/// An empty `XDG_CONFIG_HOME` is treated as unset, so the home user file must
/// still outrank a valid system file.
#[test]
fn empty_xdg_config_home_falls_back_to_home_user_file() -> Result<()> {
    let case = Case::new()?;
    let user_config = case.home.join(".netsuke.toml");
    test_support::fs::write(&user_config, "jobs = 4\n").context("write user config")?;
    let env = case.with_system_scope(
        TestEnv::default()
            .with_var("HOME", case.home.as_os_str())
            .with_var("XDG_CONFIG_HOME", ""),
    );

    assert_eq!(case.discover(&env)?, canonical(&user_config)?);
    Ok(())
}

/// `HOME` outranks `USERPROFILE` when both name a user configuration file.
#[test]
fn home_outranks_userprofile_for_user_candidates() -> Result<()> {
    let case = Case::new()?;
    let home_config = case.home.join(".netsuke.toml");
    test_support::fs::write(&home_config, "jobs = 4\n").context("write home config")?;
    let profile = case.root().join("profile");
    test_support::fs::create_dir(&profile).context("create profile directory")?;
    test_support::fs::write(profile.join(".netsuke.toml"), "jobs = 7\n")
        .context("write profile config")?;
    let env = case.with_system_scope(
        TestEnv::default()
            .with_var("HOME", case.home.as_os_str())
            .with_var("USERPROFILE", profile.as_os_str()),
    );

    assert_eq!(case.discover(&env)?, canonical(&home_config)?);
    Ok(())
}
