//! Validate Dependabot coverage for repository dependency manifests.

use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct DependabotConfig {
    version: u64,
    updates: Vec<DependabotUpdate>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct DependabotUpdate {
    package_ecosystem: String,
    directory: Option<String>,
    directories: Option<Vec<String>>,
    open_pull_requests_limit: u64,
    labels: Vec<String>,
    schedule: DependabotSchedule,
}

#[derive(Debug, Deserialize)]
struct DependabotSchedule {
    interval: String,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn dependabot_config() -> Result<DependabotConfig> {
    let path = repo_root().join(".github").join("dependabot.yml");
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("read Dependabot config from {}", path.display()))?;
    serde_yaml::from_str(&contents).context("parse Dependabot config")
}

fn update_for<'a>(config: &'a DependabotConfig, ecosystem: &str) -> Result<&'a DependabotUpdate> {
    let matches = config
        .updates
        .iter()
        .filter(|update| update.package_ecosystem == ecosystem)
        .collect::<Vec<_>>();
    ensure!(
        matches.len() == 1,
        "expected exactly one {ecosystem} update block, found {}",
        matches.len()
    );
    matches
        .into_iter()
        .next()
        .with_context(|| format!("{ecosystem} update block should exist"))
}

fn update_directories(update: &DependabotUpdate) -> Result<BTreeSet<&str>> {
    match (&update.directory, &update.directories) {
        (Some(directory), None) => Ok(BTreeSet::from([directory.as_str()])),
        (None, Some(directories)) => Ok(directories.iter().map(String::as_str).collect()),
        (None, None) => Err(anyhow::anyhow!(
            "{} should define directory or directories",
            update.package_ecosystem
        )),
        (Some(_), Some(_)) => Err(anyhow::anyhow!(
            "{} should not define both directory and directories",
            update.package_ecosystem
        )),
    }
}

fn assert_update_policy(
    update: &DependabotUpdate,
    interval: &str,
    labels: &[&str],
    open_pull_requests_limit: u64,
) {
    assert_eq!(
        update.schedule.interval, interval,
        "{} schedule interval should be {interval}",
        update.package_ecosystem
    );
    assert_eq!(
        update.open_pull_requests_limit, open_pull_requests_limit,
        "{} open pull request limit should be {open_pull_requests_limit}",
        update.package_ecosystem
    );
    assert_eq!(
        update.labels,
        labels
            .iter()
            .map(|label| String::from(*label))
            .collect::<Vec<_>>(),
        "{} labels should match repository policy",
        update.package_ecosystem
    );
}

fn repo_dir(root: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(root)
        .with_context(|| format!("{} should be under {}", path.display(), root.display()))?;
    if relative.as_os_str().is_empty() {
        return Ok(String::from("/"));
    }
    Ok(format!(
        "/{}",
        relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/")
    ))
}

fn should_skip_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, ".git" | "target"))
}

fn collect_dirs_containing_file(
    root: &Path,
    current: &Path,
    file_name: &str,
    dirs: &mut BTreeSet<String>,
) -> Result<()> {
    for dir_entry in
        fs::read_dir(current).with_context(|| format!("read directory {}", current.display()))?
    {
        let entry = dir_entry.with_context(|| format!("read entry under {}", current.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("read file type for {}", path.display()))?;
        if file_type.is_dir() {
            if !should_skip_dir(&path) {
                collect_dirs_containing_file(root, &path, file_name, dirs)?;
            }
            continue;
        }
        if file_type.is_file() && path.file_name().and_then(|name| name.to_str()) == Some(file_name)
        {
            let parent = path
                .parent()
                .with_context(|| format!("{} should have a parent directory", path.display()))?;
            dirs.insert(repo_dir(root, parent)?);
        }
    }
    Ok(())
}

fn cargo_lock_dirs(root: &Path) -> Result<BTreeSet<String>> {
    let mut lock_dirs = BTreeSet::new();
    collect_dirs_containing_file(root, root, "Cargo.lock", &mut lock_dirs)?;
    Ok(lock_dirs
        .into_iter()
        .filter(|dir| {
            let manifest_dir = if dir == "/" {
                root.to_path_buf()
            } else {
                root.join(dir.trim_start_matches('/'))
            };
            manifest_dir.join("Cargo.toml").is_file()
        })
        .collect())
}

fn workflow_files_exist(root: &Path) -> bool {
    root.join(".github")
        .join("workflows")
        .read_dir()
        .is_ok_and(|entries| {
            entries.filter_map(Result::ok).any(|entry| {
                matches!(
                    entry
                        .path()
                        .extension()
                        .and_then(|extension| extension.to_str()),
                    Some("yml" | "yaml")
                )
            })
        })
}

fn local_action_manifest_dirs(root: &Path) -> Result<BTreeSet<String>> {
    let mut action_dirs = BTreeSet::new();
    collect_dirs_containing_file(
        root,
        &root.join(".github").join("actions"),
        "action.yml",
        &mut action_dirs,
    )?;
    collect_dirs_containing_file(
        root,
        &root.join(".github").join("actions"),
        "action.yaml",
        &mut action_dirs,
    )?;
    Ok(action_dirs)
}

fn directory_pattern_matches(pattern: &str, directory: &str) -> bool {
    let Some(prefix) = pattern.strip_suffix("/*") else {
        return pattern == directory;
    };
    let Some(rest) = directory.strip_prefix(prefix) else {
        return false;
    };
    rest.strip_prefix('/')
        .is_some_and(|suffix| !suffix.is_empty() && !suffix.contains('/'))
}

fn covered_by_any_directory(directory: &str, configured_directories: &BTreeSet<&str>) -> bool {
    configured_directories
        .iter()
        .any(|pattern| directory_pattern_matches(pattern, directory))
}

#[test]
fn dependabot_updates_have_expected_policy() -> Result<()> {
    let config = dependabot_config()?;
    ensure!(config.version == 2, "Dependabot config version should be 2");
    ensure!(
        config.updates.len() == 2,
        "Dependabot config should define GitHub Actions and Cargo updates"
    );

    assert_update_policy(
        update_for(&config, "github-actions")?,
        "weekly",
        &["dependencies", "github-actions"],
        5,
    );
    assert_update_policy(
        update_for(&config, "cargo")?,
        "daily",
        &["dependencies", "cargo"],
        5,
    );
    Ok(())
}

#[test]
fn cargo_update_directories_match_lockfile_manifests() -> Result<()> {
    let config = dependabot_config()?;
    let cargo_update = update_for(&config, "cargo")?;
    let configured_directory_refs = update_directories(cargo_update)?;
    let expected_directories = cargo_lock_dirs(&repo_root())?;
    let configured_directories = configured_directory_refs
        .into_iter()
        .map(String::from)
        .collect::<BTreeSet<_>>();

    ensure!(
        configured_directories == expected_directories,
        "Cargo Dependabot directories should match checked-in Cargo lockfile manifest directories: configured={configured_directories:?}, expected={expected_directories:?}"
    );
    Ok(())
}

#[test]
fn github_actions_update_directories_cover_workflows_and_local_actions() -> Result<()> {
    let root = repo_root();
    let config = dependabot_config()?;
    let github_actions_update = update_for(&config, "github-actions")?;
    let configured_directories = update_directories(github_actions_update)?;

    ensure!(
        configured_directories.contains("/"),
        "GitHub Actions Dependabot config should include / for workflow files"
    );
    ensure!(
        workflow_files_exist(&root),
        "repository should contain GitHub workflow files covered by /"
    );

    let action_manifest_dirs = local_action_manifest_dirs(&root)?;
    ensure!(
        !action_manifest_dirs.is_empty(),
        "repository should contain at least one local action manifest"
    );
    for action_dir in &action_manifest_dirs {
        ensure!(
            covered_by_any_directory(action_dir, &configured_directories),
            "local action manifest directory {action_dir} should be covered by Dependabot"
        );
    }

    for configured_dir in configured_directories
        .iter()
        .filter(|directory| directory.starts_with("/.github/actions"))
    {
        ensure!(
            action_manifest_dirs
                .iter()
                .any(|action_dir| directory_pattern_matches(configured_dir, action_dir)),
            "configured GitHub Actions directory {configured_dir} should match a local action manifest"
        );
    }
    Ok(())
}
