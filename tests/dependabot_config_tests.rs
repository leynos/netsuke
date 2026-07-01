//! Validate Dependabot coverage for repository dependency manifests.

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use serde::Deserialize;
use std::collections::BTreeSet;

/// Parsed Dependabot configuration root.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct DependabotConfig {
    version: u64,
    updates: Vec<DependabotUpdate>,
}

/// Parsed Dependabot update entry for one package ecosystem.
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

/// Parsed Dependabot schedule block.
#[derive(Debug, Deserialize)]
struct DependabotSchedule {
    interval: String,
}

/// Return the repository root as a UTF-8 Camino path.
fn repo_root_path() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Open the repository root as a capability-scoped UTF-8 directory.
fn repo_dir() -> Result<Dir> {
    let root = repo_root_path();
    Dir::open_ambient_dir(&root, ambient_authority())
        .with_context(|| format!("open repository root {root}"))
}

/// Load and parse the repository Dependabot configuration.
fn dependabot_config() -> Result<DependabotConfig> {
    let path = Utf8Path::new(".github").join("dependabot.yml");
    let contents = repo_dir()?
        .read_to_string(&path)
        .with_context(|| format!("read Dependabot config from {path}"))?;
    serde_yaml::from_str(&contents).context("parse Dependabot config")
}

/// Find the single update entry for a package ecosystem.
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

/// Return a normalized set of configured directories for one update block.
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

/// Assert the shared Dependabot policy fields for one update block.
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

/// Convert a relative repository path to the POSIX directory form Dependabot uses.
fn dependabot_dir_from_relative(relative: &Utf8Path) -> String {
    let components = relative
        .components()
        .filter_map(|component| {
            let component_text = component.as_str();
            (component_text != ".").then_some(component_text)
        })
        .collect::<Vec<_>>();
    if components.is_empty() {
        return String::from("/");
    }
    format!("/{}", components.join("/"))
}

/// Return whether a repository traversal should skip a directory name.
fn should_skip_dir(name: &str) -> bool {
    matches!(name, ".git" | "target")
}

/// Collect Dependabot directory names containing a file with the given name.
fn collect_dirs_containing_file(
    root: &Dir,
    current: &Utf8Path,
    file_name: &str,
    dirs: &mut BTreeSet<String>,
) -> Result<()> {
    let mut search = DirectorySearch {
        root,
        file_name,
        dirs,
    };
    for dir_entry in root
        .read_dir(current)
        .with_context(|| format!("read directory {current}"))?
    {
        handle_dir_entry(&mut search, current, dir_entry)?;
    }
    Ok(())
}

/// Shared traversal state for a manifest-discovery pass.
struct DirectorySearch<'a> {
    root: &'a Dir,
    file_name: &'a str,
    dirs: &'a mut BTreeSet<String>,
}

/// Handle one directory entry during manifest discovery.
fn handle_dir_entry(
    search: &mut DirectorySearch<'_>,
    current: &Utf8Path,
    dir_entry: std::io::Result<cap_std::fs_utf8::DirEntry>,
) -> Result<()> {
    let entry = dir_entry.with_context(|| format!("read entry under {current}"))?;
    let entry_name = entry
        .file_name()
        .with_context(|| format!("read entry name under {current}"))?;
    let file_type = entry
        .file_type()
        .with_context(|| format!("read file type for {current}/{entry_name}"))?;
    if file_type.is_dir() {
        if !should_skip_dir(&entry_name) {
            collect_dirs_containing_file(
                search.root,
                &current.join(&entry_name),
                search.file_name,
                search.dirs,
            )?;
        }
        return Ok(());
    }
    if file_type.is_file() && entry_name == search.file_name {
        search.dirs.insert(dependabot_dir_from_relative(current));
    }
    Ok(())
}

/// Return Cargo manifest directories that own checked-in lockfiles.
fn cargo_lock_dirs(root: &Dir) -> Result<BTreeSet<String>> {
    let mut lock_dirs = BTreeSet::new();
    collect_dirs_containing_file(root, Utf8Path::new("."), "Cargo.lock", &mut lock_dirs)?;
    Ok(lock_dirs
        .into_iter()
        .filter(|dir| {
            let manifest_path = if dir == "/" {
                Utf8PathBuf::from("Cargo.toml")
            } else {
                Utf8Path::new(dir.trim_start_matches('/')).join("Cargo.toml")
            };
            root.is_file(manifest_path)
        })
        .collect())
}

/// Return whether the repository has at least one workflow YAML file.
fn workflow_files_exist(root: &Dir) -> Result<bool> {
    let workflows_dir = Utf8Path::new(".github").join("workflows");
    for dir_entry in root
        .read_dir(&workflows_dir)
        .with_context(|| format!("read directory {workflows_dir}"))?
    {
        let entry = dir_entry.with_context(|| format!("read entry under {workflows_dir}"))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("read file type under {workflows_dir}"))?;
        let entry_name = entry
            .file_name()
            .with_context(|| format!("read entry name under {workflows_dir}"))?;
        if file_type.is_file()
            && matches!(Utf8Path::new(&entry_name).extension(), Some("yml" | "yaml"))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Return local composite action directories that contain action manifests.
fn local_action_manifest_dirs(root: &Dir) -> Result<BTreeSet<String>> {
    let mut action_dirs = BTreeSet::new();
    let actions_dir = Utf8Path::new(".github").join("actions");
    collect_dirs_containing_file(root, &actions_dir, "action.yml", &mut action_dirs)?;
    collect_dirs_containing_file(root, &actions_dir, "action.yaml", &mut action_dirs)?;
    Ok(action_dirs)
}

/// Return whether a Dependabot directory pattern covers a concrete directory.
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

/// Return whether any configured directory covers a concrete directory.
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
    let expected_directories = cargo_lock_dirs(&repo_dir()?)?;
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
    let root = repo_dir()?;
    let config = dependabot_config()?;
    let github_actions_update = update_for(&config, "github-actions")?;
    let configured_directories = update_directories(github_actions_update)?;

    ensure!(
        configured_directories.contains("/"),
        "GitHub Actions Dependabot config should include / for workflow files"
    );
    ensure!(
        workflow_files_exist(&root)?,
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
