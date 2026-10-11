//! End-to-end trust-boundary tests for layered fetch-policy configuration.

use super::merge_probe::{environment_with_system_scope, merge_in_child};
use anyhow::{Context, Result, ensure};
use netsuke::cli::Cli;
use netsuke::stdlib::NetworkPolicyViolation;
use std::ffi::OsString;
use std::path::Path;
use tempfile::{TempDir, tempdir};
use test_support::fs as test_fs;
use url::Url;

/// Describe the configuration-file contents used in one merge scenario.
#[derive(Default)]
struct FetchPolicyFileLayers<'a> {
    /// Hold system-scope configuration contents.
    system: Option<&'a str>,
    /// Hold user-scope configuration contents.
    user: Option<&'a str>,
    /// Hold primary project-scope configuration contents.
    project: Option<&'a str>,
    /// Hold project-relative files selected by the project configuration.
    project_files: Option<Vec<(&'a str, &'a str)>>,
    /// Hold user-configuration-relative files selected by the user config.
    user_files: Option<Vec<(&'a str, &'a str)>>,
}

/// Write a configuration file while creating its parent directory.
fn write_config(config_path: impl AsRef<Path>, contents: &str) -> Result<()> {
    let target = config_path.as_ref();
    let parent = target
        .parent()
        .context("configuration file has no parent")?;
    test_fs::create_dir_all(parent).context("create configuration directory")?;
    test_fs::write(target, contents).with_context(|| format!("write {}", target.display()))
}

/// Merge one isolated system, user, project, environment, and CLI scenario.
fn merge_fetch_policy(
    file_layers: &FetchPolicyFileLayers<'_>,
    environment_overrides: &[(OsString, OsString)],
    args: &[&str],
) -> Result<Cli> {
    let project = tempdir().context("create project directory")?;
    merge_fetch_policy_in_project(&project, file_layers, environment_overrides, args)
}

/// Merge a fetch-policy scenario while keeping its project workspace alive.
fn merge_fetch_policy_in_project(
    project: &TempDir,
    file_layers: &FetchPolicyFileLayers<'_>,
    environment_overrides: &[(OsString, OsString)],
    args: &[&str],
) -> Result<Cli> {
    let home = tempdir().context("create home directory")?;
    let system = tempdir().context("create system directory")?;
    let project_path = project.path().to_string_lossy();
    let user_config_dir = home.path().join(".config/netsuke");
    if let Some(config) = file_layers.system {
        write_expanded_config(
            system.path().join("netsuke/config.toml"),
            config,
            &project_path,
        )?;
    }
    if let Some(config) = file_layers.user {
        write_expanded_config(user_config_dir.join("config.toml"), config, &project_path)?;
    }
    if let Some(config) = file_layers.project {
        write_expanded_config(project.path().join(".netsuke.toml"), config, &project_path)?;
    }
    write_relative_configs(
        project.path(),
        file_layers.project_files.as_deref(),
        &project_path,
    )?;
    write_relative_configs(
        &user_config_dir,
        file_layers.user_files.as_deref(),
        &project_path,
    )?;
    let mut environment = environment_with_system_scope(home.path(), system.path(), &[]);
    environment.extend_from_slice(environment_overrides);
    merge_in_child(args, project.path(), &environment)
}

/// Write configuration contents after expanding the shared project-path token.
fn write_expanded_config(
    config_path: impl AsRef<Path>,
    contents: &str,
    project_path: &str,
) -> Result<()> {
    let expanded = contents.replace("{project_dir}", project_path);
    write_config(config_path, &expanded)
}

/// Write relative files selected by one configuration root.
fn write_relative_configs(
    root: &Path,
    files: Option<&[(&str, &str)]>,
    project_path: &str,
) -> Result<()> {
    for (relative_path, contents) in files.unwrap_or_default() {
        write_expanded_config(root.join(relative_path), contents, project_path)?;
    }
    Ok(())
}

/// Merge a scenario that contains only primary project fetch-policy settings.
fn merge_project_fetch_policy(project_config: &str) -> Result<Cli> {
    merge_fetch_policy(
        &FetchPolicyFileLayers {
            project: Some(project_config),
            ..FetchPolicyFileLayers::default()
        },
        &[],
        &["netsuke"],
    )
}

/// Build a URL for a policy assertion.
fn url(value: &str) -> Result<Url> {
    Url::parse(value).with_context(|| format!("parse test URL {value}"))
}

/// Evaluate a URL and return the policy rejection it produces.
fn rejection(cli: &Cli, value: &str) -> Result<NetworkPolicyViolation> {
    cli.network_policy()?
        .evaluate(&url(value)?)
        .err()
        .with_context(|| format!("URL should be rejected by the policy: {value}"))
}

/// Assert that a URL is rejected because its host was not allowlisted.
fn assert_host_not_allowlisted(cli: &Cli, value: &str) -> Result<()> {
    let error = rejection(cli, value)?;
    ensure!(
        matches!(error, NetworkPolicyViolation::HostNotAllowlisted { .. }),
        "expected host allowlist violation, got {error:?}"
    );
    Ok(())
}

/// Assert that a URL is rejected because its scheme was not allowlisted.
fn assert_scheme_not_allowed(cli: &Cli, value: &str) -> Result<()> {
    let error = rejection(cli, value)?;
    ensure!(
        matches!(error, NetworkPolicyViolation::SchemeNotAllowed { .. }),
        "expected scheme violation, got {error:?}"
    );
    Ok(())
}

/// Assert that a URL is rejected because a block rule takes priority.
fn assert_host_blocked(cli: &Cli, value: &str) -> Result<()> {
    let error = rejection(cli, value)?;
    ensure!(
        matches!(error, NetworkPolicyViolation::HostBlocked { .. }),
        "expected host block violation, got {error:?}"
    );
    Ok(())
}

mod extends_tests;
mod fetch_tests;
mod merge_tests;
