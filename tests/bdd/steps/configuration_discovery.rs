//! Step definitions for configuration discovery scenarios.

use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use crate::bdd::types::{EnvVarKey, EnvVarValue, FileName, NamesList};
use anyhow::{Context, Result, ensure};
use netsuke::cli::config::OutputFormat;
use netsuke::theme::ThemePreference;
use rstest_bdd_macros::{given, then};
use std::fs;
use tempfile::tempdir;

#[given("a temporary workspace")]
fn a_temporary_workspace(world: &TestWorld) -> Result<()> {
    let temp = tempdir().context("failed to create temporary workspace")?;
    *world.temp_dir.borrow_mut() = Some(temp);
    Ok(())
}

/// Write `content` to `file_name` inside `world`'s temp directory.
/// Set `chdir` to `true` for project-scope configs so discovery works
/// without an explicit path override.
fn write_config_file(world: &TestWorld, file_name: &str, content: &str, chdir: bool) -> Result<()> {
    let temp_dir = world
        .temp_dir
        .borrow()
        .as_ref()
        .context("temp_dir should be set")?
        .path()
        .to_path_buf();

    let config_path = temp_dir.join(file_name);
    fs::write(&config_path, content).with_context(|| format!("failed to write {file_name}"))?;

    if chdir {
        // Acquire scenario-scoped lock before process-global CWD mutation
        world.ensure_env_lock();
        std::env::set_current_dir(&temp_dir).context("failed to change to temp directory")?;
    }

    Ok(())
}

#[given("a project config file {file_name:string} with theme {theme:string} and jobs {jobs}")]
fn project_config_with_theme_and_jobs(
    world: &TestWorld,
    file_name: FileName,
    theme: ThemePreference,
    jobs: u32,
) -> Result<()> {
    let content = format!(
        r#"
theme = "{theme}"
jobs = {jobs}
"#
    );
    write_config_file(world, file_name.as_str(), &content, true)
}

#[given("a project config file {file_name:string} with theme {theme:string}")]
fn project_config_with_theme(
    world: &TestWorld,
    file_name: FileName,
    theme: ThemePreference,
) -> Result<()> {
    let content = format!(
        r#"
theme = "{theme}"
"#
    );
    write_config_file(world, file_name.as_str(), &content, true)
}

#[given(
    "a project config file {file_name:string} with theme {theme:string} and output format {format:string}"
)]
fn project_config_with_theme_and_format(
    world: &TestWorld,
    file_name: FileName,
    theme: ThemePreference,
    format: OutputFormat,
) -> Result<()> {
    let content = format!(
        r#"
theme = "{theme}"
output_format = "{format}"
"#
    );
    write_config_file(world, file_name.as_str(), &content, true)
}

#[given("a project config file {file_name:string} with default targets {targets:string}")]
fn project_config_with_default_targets(
    world: &TestWorld,
    file_name: FileName,
    targets: NamesList,
) -> Result<()> {
    // Parse comma-separated targets into TOML array format
    let targets_toml = format!(
        "[{}]",
        targets
            .iter()
            .map(|t| format!("\"{t}\""))
            .collect::<Vec<_>>()
            .join(", ")
    );

    let content = format!(
        r"
default_targets = {targets_toml}
"
    );
    write_config_file(world, file_name.as_str(), &content, true)
}

#[given("a custom config file {file_name:string} with theme {theme:string}")]
fn custom_config_with_theme(
    world: &TestWorld,
    file_name: FileName,
    theme: ThemePreference,
) -> Result<()> {
    let content = format!(
        r#"
theme = "{theme}"
"#
    );
    write_config_file(world, file_name.as_str(), &content, false)
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "BDD step functions must return Result<()>"
)]
#[given("the environment variable {var_name:string} is set to {value:string}")]
fn env_var_is_set(world: &TestWorld, var_name: EnvVarKey, value: EnvVarValue) -> Result<()> {
    // Acquire scenario-scoped lock before process-global env mutation
    world.ensure_env_lock();

    let original = std::env::var_os(var_name.as_str());

    // SAFETY: EnvLock (held via world.env_lock) serialises mutations
    unsafe {
        std::env::set_var(var_name.as_str(), value.as_str());
    }

    // Track for cleanup
    world
        .env_vars
        .borrow_mut()
        .insert(var_name.into_string(), original);

    Ok(())
}

#[given("the environment variable {var_name:string} points to {file_name:string}")]
fn env_var_points_to_file(
    world: &TestWorld,
    var_name: EnvVarKey,
    file_name: FileName,
) -> Result<()> {
    let temp_dir = world
        .temp_dir
        .borrow()
        .as_ref()
        .context("temp_dir should be set")?
        .path()
        .to_path_buf();

    let file_path = temp_dir.join(file_name.as_str());

    // Acquire scenario-scoped lock before process-global env mutation
    world.ensure_env_lock();

    let original = std::env::var_os(var_name.as_str());

    // SAFETY: EnvLock (held via world.env_lock) serialises mutations
    unsafe {
        std::env::set_var(var_name.as_str(), file_path.as_os_str());
    }

    world
        .env_vars
        .borrow_mut()
        .insert(var_name.into_string(), original);

    Ok(())
}

#[then("the theme preference is {expected:string}")]
fn theme_preference_is(world: &TestWorld, expected: ThemePreference) -> Result<()> {
    let actual = world
        .cli
        .with_ref(|cli| cli.theme)
        .flatten()
        .context("CLI theme should be present")?;

    ensure!(
        actual == expected,
        "expected theme {expected:?}, got {actual:?}"
    );
    Ok(())
}

#[then("the jobs setting is {expected}")]
fn jobs_setting_is(world: &TestWorld, expected: u32) -> Result<()> {
    let actual = world
        .cli
        .with_ref(|cli| cli.jobs)
        .flatten()
        .context("CLI jobs should be present")?;

    ensure!(
        u32::try_from(actual)? == expected,
        "expected jobs {expected}, got {actual}"
    );
    Ok(())
}
