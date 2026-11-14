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
use rstest::rstest;
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

/// How a case names the user's home directory to the child discovery pass.
#[derive(Clone, Copy, Debug)]
enum HomeArrangement {
    /// Only `USERPROFILE` is set, so home resolution must fall back to it.
    UserProfileOnly,
    /// `HOME` is set and `XDG_CONFIG_HOME` is empty, which discovery treats as
    /// unset.
    EmptyXdgConfigHome,
    /// `HOME` and `USERPROFILE` name different roots, and `HOME` must win.
    HomeAndUserProfile,
}

/// Isolated project, home, profile, and system roots for one user-scope case.
///
/// The system root always holds a valid configuration, so a case that passes
/// only because the system file went unread shows up as the wrong layer
/// winning. The profile root holds a second user file whose values differ from
/// the home dotfile's, so a case cannot pass by reading the wrong one.
struct Case {
    /// Keeps the scenario directories alive for the whole case.
    _temp: TempDir,
    cli: Cli,
    home: PathBuf,
    profile: PathBuf,
    system: PathBuf,
}

impl Case {
    /// Build the isolated roots and their configuration files.
    fn new() -> Result<Self> {
        let temp = tempdir().context("create temp dir")?;
        let cli = project_cli(temp.path())?;
        let home = temp.path().join("home");
        let profile = temp.path().join("profile");
        for directory in [&home, &profile] {
            test_support::fs::create_dir(directory)
                .with_context(|| format!("create {}", directory.display()))?;
        }
        test_support::fs::write(profile.join(".netsuke.toml"), "jobs = 7\n")
            .context("write profile config")?;
        let system = temp.path().join("system");
        write_system_config(&system)?;
        Ok(Self {
            _temp: temp,
            cli,
            home,
            profile,
            system,
        })
    }

    /// Write `contents` to `$HOME/.netsuke.toml` and return its path.
    fn write_home_dotfile(&self, contents: &str) -> Result<PathBuf> {
        let path = self.home.join(".netsuke.toml");
        test_support::fs::write(&path, contents)
            .with_context(|| format!("write home config {}", path.display()))?;
        Ok(path)
    }

    /// Build the discovery environment for `arrangement` with the system root
    /// redirected at this case's system directory.
    fn environment(&self, arrangement: HomeArrangement) -> TestEnv {
        let user_variables = match arrangement {
            HomeArrangement::UserProfileOnly => {
                TestEnv::default().with_var("USERPROFILE", self.home.as_os_str())
            }
            HomeArrangement::EmptyXdgConfigHome => TestEnv::default()
                .with_var("HOME", self.home.as_os_str())
                .with_var("XDG_CONFIG_HOME", ""),
            HomeArrangement::HomeAndUserProfile => TestEnv::default()
                .with_var("HOME", self.home.as_os_str())
                .with_var("USERPROFILE", self.profile.as_os_str()),
        };
        user_variables.with_var("XDG_CONFIG_DIRS", self.system.as_os_str())
    }

    /// Discover from `env` and require `expected` to be the only layer.
    fn assert_single_layer(&self, env: &TestEnv, expected: &Path) -> Result<()> {
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
        let recorded = layers
            .first()
            .context("discovered layer should be present")?
            .path()
            .map(|path| path.as_str().to_owned());
        assert_eq!(recorded, Some(canonical(expected)?));
        Ok(())
    }
}

/// A user file must outrank a valid system file however the user root reaches
/// discovery.
///
/// The `USERPROFILE` case proves the home fallback works when `HOME` is
/// absent; the empty `XDG_CONFIG_HOME` case proves an empty value is treated
/// as unset; the two-root case proves `HOME` outranks `USERPROFILE`.
#[rstest]
#[case::userprofile_only(HomeArrangement::UserProfileOnly)]
#[case::empty_xdg_config_home(HomeArrangement::EmptyXdgConfigHome)]
#[case::home_outranks_userprofile(HomeArrangement::HomeAndUserProfile)]
fn home_user_file_outranks_system_scope(#[case] arrangement: HomeArrangement) -> Result<()> {
    let case = Case::new()?;
    let home_config = case.write_home_dotfile("jobs = 4\n")?;

    case.assert_single_layer(&case.environment(arrangement), &home_config)
}
