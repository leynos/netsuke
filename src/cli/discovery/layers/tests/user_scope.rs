//! User-scope candidate derivation and selection tests.
//!
//! These pin the Netsuke-owned user pass that must consider every user
//! location before the dependency's system scope. Each candidate case supplies
//! a valid system file so a regression that restores the dependency's flat
//! order is observable as the system layer winning.

use super::super::super::paths::{FsPathNormalizer, normalized_path_key, user_scope_candidates};
use super::super::super::{discover_file_layers, discovery_env_source};
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
pub(super) fn canonical(path: &Path) -> Result<String> {
    Ok(
        normalized_path_key(&FsPathNormalizer, &path.to_string_lossy())
            .with_context(|| format!("canonicalise {}", path.display()))?
            .to_string_lossy()
            .into_owned(),
    )
}

/// Write `contents` to `path`, creating any missing parent directory.
pub(super) fn write_config(path: &Path, contents: &str) -> Result<PathBuf> {
    test_support::fs::create_dir_all(path.parent().context("configuration path has a parent")?)
        .with_context(|| format!("create configuration directory for {}", path.display()))?;
    test_support::fs::write(path, contents)
        .with_context(|| format!("write configuration {}", path.display()))?;
    Ok(path.to_path_buf())
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

/// Which Windows user root a case sets.
///
/// The candidates are derived from the injected environment, so these cases run
/// on every platform rather than behind a `cfg(windows)` gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WindowsRoot {
    /// The roaming application data root.
    AppData,
    /// The local application data root.
    LocalAppData,
}

impl WindowsRoot {
    /// Return the environment variable naming this root.
    fn variable(self) -> &'static str {
        match self {
            Self::AppData => "APPDATA",
            Self::LocalAppData => "LOCALAPPDATA",
        }
    }

    /// Return the scenario directory name holding this root's configuration.
    fn directory(self) -> &'static str {
        match self {
            Self::AppData => "appdata",
            Self::LocalAppData => "localappdata",
        }
    }
}

/// Isolated project, home, profile, and system roots for one user-scope case.
///
/// The system root always holds a valid configuration, so a case that passes
/// only because the system file went unread shows up as the wrong layer
/// winning. The profile root holds a second user file whose values differ from
/// the home dotfile's, so a case cannot pass by reading the wrong one.
pub(super) struct Case {
    /// Keeps the scenario directories alive for the whole case.
    _temp: TempDir,
    cli: Cli,
    home: PathBuf,
    profile: PathBuf,
    system: PathBuf,
}

impl Case {
    /// Build the isolated roots and their configuration files.
    pub(super) fn new() -> Result<Self> {
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

    /// Return the user's home directory for this case.
    pub(super) fn home(&self) -> &Path {
        &self.home
    }

    /// Return the system configuration file this case installed.
    pub(super) fn system_config(&self) -> PathBuf {
        self.system.join("netsuke/config.toml")
    }

    /// Create the case directory named `name` and return its path.
    pub(super) fn base(&self, name: &str) -> Result<PathBuf> {
        let base = self._temp.path().join(name);
        test_support::fs::create_dir(&base)
            .with_context(|| format!("create {}", base.display()))?;
        Ok(base)
    }

    /// Write `contents` to `$HOME/.netsuke.toml` and return its path.
    fn write_home_dotfile(&self, contents: &str) -> Result<PathBuf> {
        write_config(&self.home.join(".netsuke.toml"), contents)
    }

    /// Redirect the system scope at this case's system root.
    pub(super) fn scoped_environment(&self, user_variables: TestEnv) -> TestEnv {
        user_variables.with_var("XDG_CONFIG_DIRS", self.system.as_os_str())
    }

    /// Build the discovery environment for `arrangement`.
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
        self.scoped_environment(user_variables)
    }

    /// Discover from `env`, returning the recorded layer paths and first error.
    pub(super) fn discover(&self, env: &TestEnv) -> (Vec<String>, Option<String>) {
        let discovered = discover_file_layers(&self.cli, env);
        let paths = discovered
            .layers()
            .iter()
            .filter_map(|layer| layer.path().map(|path| path.as_str().to_owned()))
            .collect();
        let error = discovered.first_error().map(ToString::to_string);
        (paths, error)
    }

    /// Discover from `env` and require `expected` to be the only layer.
    fn assert_single_layer(&self, env: &TestEnv, expected: &Path) -> Result<()> {
        let (paths, error) = self.discover(env);
        ensure!(
            error.is_none(),
            "injected discovery should succeed: {}",
            error.unwrap_or_default()
        );
        ensure!(
            paths.len() == 1,
            "expected exactly one discovered layer, got {paths:?}"
        );
        let expected_key = canonical(expected)?;
        assert_eq!(paths.first(), Some(&expected_key));
        Ok(())
    }

    /// Require discovery to fail with an error naming `expected_fragment`.
    fn assert_discovery_error(&self, env: &TestEnv, expected_fragment: &str) -> Result<()> {
        let (paths, error) = self.discover(env);
        ensure!(
            paths.is_empty(),
            "a rejected user candidate must not fall through to another layer, got {paths:?}"
        );
        let error = error.context("the user candidate should fail discovery")?;
        ensure!(
            error.contains(expected_fragment),
            "the error should name {expected_fragment}: {error}"
        );
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

/// Both Windows user roots outrank the system scope, and `APPDATA` outranks
/// `LOCALAPPDATA`.
///
/// The two-root case lists `LOCALAPPDATA` first, so the winner can only come
/// from the documented candidate order rather than from the order a case
/// supplies.
#[rstest]
#[case::appdata_alone(&[WindowsRoot::AppData], WindowsRoot::AppData)]
#[case::localappdata_alone(&[WindowsRoot::LocalAppData], WindowsRoot::LocalAppData)]
#[case::appdata_outranks_localappdata(
    &[WindowsRoot::LocalAppData, WindowsRoot::AppData],
    WindowsRoot::AppData
)]
fn windows_user_roots_outrank_system_scope(
    #[case] roots: &[WindowsRoot],
    #[case] winner: WindowsRoot,
) -> Result<()> {
    let case = Case::new()?;
    let mut environment = TestEnv::default();
    let mut expected = None;
    for root in roots {
        let base = case.base(root.directory())?;
        let config = write_config(&base.join("netsuke/config.toml"), "jobs = 4\n")?;
        environment = environment.with_var(root.variable(), base.into_os_string());
        if *root == winner {
            expected = Some(config);
        }
    }

    let expected = expected.context("the winning root should be configured")?;
    case.assert_single_layer(&case.scoped_environment(environment), &expected)
}

/// An empty base value is ignored, so no candidate is relative to the working
/// directory.
///
/// A relative candidate would load configuration from wherever the tool
/// happened to run rather than from a user location. The surviving
/// `USERPROFILE` base keeps the list non-empty, so removing the filter shows up
/// as a relative candidate or a candidate outside the home directory.
#[rstest]
#[case::xdg_config_home("XDG_CONFIG_HOME")]
#[case::appdata("APPDATA")]
#[case::localappdata("LOCALAPPDATA")]
#[case::home("HOME")]
fn empty_base_value_is_ignored(#[case] empty_variable: &'static str) -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let home = temp.path().join("home");
    let environment = TestEnv::default()
        .with_var(empty_variable, "")
        .with_var("USERPROFILE", home.as_os_str());

    let candidates = user_scope_candidates(discovery_env_source(&environment).as_ref());

    ensure!(
        !candidates.is_empty(),
        "the surviving user base should still yield candidates"
    );
    for candidate in &candidates {
        ensure!(
            candidate.is_absolute(),
            "a candidate must not resolve against the working directory: {candidate:?}"
        );
        ensure!(
            candidate.starts_with(&home),
            "a candidate should sit beneath the surviving home base: {candidate:?}"
        );
    }
    Ok(())
}

/// A user file that exists but does not resolve stops discovery instead of
/// falling through to the system file.
///
/// `OrthoConfig` resolves a candidate through its parent directory and reports
/// a target that does not resolve as absent, exactly as it reports a file that
/// is not there. Without a separate presence probe, a dangling
/// `$HOME/.netsuke.toml` would hand the system scope the fetch and
/// `env_allow_var` policy the user meant to override.
#[cfg(unix)]
#[rstest]
fn dangling_user_dotfile_symlink_stops_discovery() -> Result<()> {
    let case = Case::new()?;
    let link = case.home.join(".netsuke.toml");
    // The target is relative so the link stays inside the home directory.
    // `ortho_config` resolves candidates through a capability on the parent
    // directory, which rejects an absolute link escaping it outright; only a
    // link that stays inside is reported as an absent file.
    test_support::fs::symlink("absent-target.toml", &link)
        .context("create dangling home symlink")?;

    let environment = case.environment(HomeArrangement::EmptyXdgConfigHome);
    case.assert_discovery_error(&environment, ".netsuke.toml")
}

/// A user symlink whose target exists loads that target.
///
/// Rejecting a broken link must not reject every link: discovery resolves the
/// recorded layer to the target, which is the file whose contents are read.
#[cfg(unix)]
#[rstest]
fn valid_user_dotfile_symlink_loads_its_target() -> Result<()> {
    let case = Case::new()?;
    let target = write_config(&case.home.join("shared-config.toml"), "jobs = 4\n")?;
    let link = case.home.join(".netsuke.toml");
    test_support::fs::symlink("shared-config.toml", &link).context("create home symlink")?;

    let environment = case.environment(HomeArrangement::EmptyXdgConfigHome);
    case.assert_single_layer(&environment, &target)
}

/// A genuinely absent user candidate is skipped, so system configuration still
/// loads when the user has none.
///
/// This is the companion of the dangling-symlink case: rejecting a broken entry
/// must not turn "nothing installed" into a failure.
#[rstest]
fn absent_user_dotfile_falls_through_to_system_scope() -> Result<()> {
    let case = Case::new()?;

    let environment = case.environment(HomeArrangement::EmptyXdgConfigHome);
    case.assert_single_layer(&environment, &case.system_config())
}
