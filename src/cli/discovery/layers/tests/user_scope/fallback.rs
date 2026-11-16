//! Fallback-environment tests for the user-scope base rule.
//!
//! The user pass rejects an empty or non-absolute base, but `ortho_config`
//! 0.10.0 filters only empty values. These tests pin both halves of the rule:
//! the candidate derivation ignores such a value, and the environment view
//! handed to the dependency's own scan hides it, so it cannot return as a
//! working-directory relative candidate.

use crate::cli::discovery::discovery_env_source;
use crate::cli::discovery::layers::config_discovery;
use crate::cli::discovery::paths::{SanitisedUserBases, user_scope_candidates};
use anyhow::{Context, Result, ensure};
use ortho_config::{MapEnv, SharedEnvSource};
use rstest::rstest;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::tempdir;

use super::{Case, TestEnv, write_config};

/// Return the dependency's candidate list for `env_source`.
fn candidates(env_source: SharedEnvSource) -> Vec<PathBuf> {
    config_discovery(None, env_source).candidates()
}

/// An unusable base value reads as unset in the sanitised view.
///
/// Each case names the variable on the test rather than reading a shared list,
/// so dropping a variable from the production list leaves this test short of a
/// case instead of silently shrinking what it covers.
#[rstest]
#[case::empty("")]
#[case::relative("relative-base")]
#[case::drive_relative_root("\\config")]
fn unusable_value_reads_as_unset(
    #[case] value: &'static str,
    #[values("XDG_CONFIG_HOME", "APPDATA", "LOCALAPPDATA", "HOME", "USERPROFILE")]
    key: &'static str,
) {
    let source: SharedEnvSource = Arc::new(SanitisedUserBases::new(Arc::new(
        MapEnv::new().with_var(key, value),
    )));
    let lookup = source.get(key);

    assert!(
        lookup.is_none(),
        "{key}={value:?} must read as unset, got {lookup:?}"
    );
}

/// An absolute base value passes through the sanitised view unchanged.
#[rstest]
fn absolute_value_passes_through(
    #[values("XDG_CONFIG_HOME", "APPDATA", "LOCALAPPDATA", "HOME", "USERPROFILE")]
    key: &'static str,
) {
    let absolute = OsString::from("/config");
    let source: SharedEnvSource = Arc::new(SanitisedUserBases::new(Arc::new(
        MapEnv::new().with_var(key, &absolute),
    )));

    assert_eq!(
        source.get(key),
        Some(absolute),
        "{key} should keep its absolute value"
    );
}

/// A relative base cannot enter the dependency's own candidate list.
///
/// This is the discriminating case. The dependency admits a relative value, so
/// the first assertion fails if that behaviour changes and the case would stop
/// proving anything. The later assertions fail if the fallback scan is handed
/// the raw environment, because the relative base then becomes a
/// working-directory relative user candidate that could supply configuration
/// and the policy it carries.
#[test]
fn relative_base_cannot_enter_the_fallback_scan() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let system = temp.path().join("system");
    let system_file = write_config(&system.join("netsuke/config.toml"), "jobs = 9\n")?;
    let home = temp.path().join("home");
    test_support::fs::create_dir(&home).context("create home directory")?;
    let variables = MapEnv::new()
        .with_var("XDG_CONFIG_HOME", "relative-base")
        .with_var("HOME", home.as_os_str())
        .with_var("XDG_CONFIG_DIRS", system.as_os_str());

    let raw: SharedEnvSource = Arc::new(variables.clone());
    let leaking = candidates(raw);
    ensure!(
        leaking
            .iter()
            .any(|candidate| candidate == Path::new("relative-base/netsuke/config.toml")),
        "the dependency should admit a relative base, or this case cannot discriminate: {leaking:?}"
    );

    let sanitised: SharedEnvSource = Arc::new(SanitisedUserBases::new(Arc::new(variables)));
    let candidates = candidates(sanitised);

    ensure!(
        candidates.iter().all(|candidate| candidate.is_absolute()),
        "no fallback candidate may resolve against the working directory: {candidates:?}"
    );
    ensure!(
        candidates.contains(&system_file),
        "the system candidate should survive the sanitised view: {candidates:?}"
    );
    Ok(())
}

/// A base value that is not absolute is ignored, so no candidate depends on
/// process state.
///
/// A relative candidate would load configuration from wherever the tool
/// happened to run rather than from a user location, and its grants would be
/// merged with operator authority. A drive-relative rooted value such as
/// `\config` is the same hazard on Windows: it resolves on whichever drive the
/// process happens to run from. The surviving `USERPROFILE` base keeps the list
/// non-empty, so a weakened filter shows up as a non-absolute candidate or one
/// outside the home directory.
#[rstest]
#[case::xdg_config_home_empty("XDG_CONFIG_HOME", "")]
#[case::xdg_config_home_relative("XDG_CONFIG_HOME", "relative-base")]
#[case::xdg_config_home_drive_relative_root("XDG_CONFIG_HOME", "\\config")]
#[case::appdata_empty("APPDATA", "")]
#[case::appdata_relative("APPDATA", "relative-base")]
#[case::appdata_drive_relative_root("APPDATA", "\\config")]
#[case::localappdata_empty("LOCALAPPDATA", "")]
#[case::localappdata_relative("LOCALAPPDATA", "relative-base")]
#[case::home_empty("HOME", "")]
#[case::home_relative("HOME", "relative-base")]
#[case::home_drive_relative_root("HOME", "\\config")]
fn unusable_base_value_is_ignored(
    #[case] variable: &'static str,
    #[case] value: &'static str,
) -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let home = temp.path().join("home");
    let environment = TestEnv::default()
        .with_var(variable, value)
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

/// A relative user base contributes no layer to either pass.
///
/// The surviving `USERPROFILE` base holds no user file, so the system file is
/// the only layer that can load. Both the user pass and the dependency's own
/// scan see the relative value as unset, so nothing that the working directory
/// happened to contain could be merged with operator authority.
#[rstest]
#[case::xdg_config_home("XDG_CONFIG_HOME")]
#[case::appdata("APPDATA")]
#[case::localappdata("LOCALAPPDATA")]
#[case::home("HOME")]
fn relative_base_contributes_no_layer(#[case] variable: &'static str) -> Result<()> {
    let case = Case::new()?;
    let environment = TestEnv::default()
        .with_var(variable, "relative-base")
        .with_var("USERPROFILE", case.home().as_os_str());

    case.assert_single_layer(&case.scoped_environment(environment), &case.system_config())
}
