//! Step definitions for configuration discovery scenarios.

use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use anyhow::{Context, Result, ensure};
use netsuke::theme::ThemePreference;
use rstest_bdd_macros::{given, then};
use std::fs;
use tempfile::tempdir;
use test_support::env_lock::EnvLock;

#[given("a temporary workspace")]
fn a_temporary_workspace(world: &TestWorld) -> Result<()> {
    let temp = tempdir().context("failed to create temporary workspace")?;
    *world.temp_dir.borrow_mut() = Some(temp);
    Ok(())
}

#[given("a project config file {file_name:string} with theme {theme:string} and jobs {jobs}")]
fn project_config_with_theme_and_jobs(
    world: &TestWorld,
    file_name: String,
    theme: String,
    jobs: u32,
) -> Result<()> {
    let temp_dir = world
        .temp_dir
        .borrow()
        .as_ref()
        .context("temp_dir should be set")?
        .path()
        .to_path_buf();

    let config_path = temp_dir.join(&file_name);
    let config_content = format!(
        r#"
theme = "{theme}"
jobs = {jobs}
"#
    );
    fs::write(&config_path, config_content)
        .with_context(|| format!("failed to write {file_name}"))?;

    // Change to temp directory so config is discovered
    std::env::set_current_dir(&temp_dir).context("failed to change to temp directory")?;

    Ok(())
}

#[given("a project config file {file_name:string} with theme {theme:string}")]
fn project_config_with_theme(world: &TestWorld, file_name: String, theme: String) -> Result<()> {
    let temp_dir = world
        .temp_dir
        .borrow()
        .as_ref()
        .context("temp_dir should be set")?
        .path()
        .to_path_buf();

    let config_path = temp_dir.join(&file_name);
    let config_content = format!(
        r#"
theme = "{theme}"
"#
    );
    fs::write(&config_path, config_content)
        .with_context(|| format!("failed to write {file_name}"))?;

    std::env::set_current_dir(&temp_dir).context("failed to change to temp directory")?;

    Ok(())
}

#[given(
    "a project config file {file_name:string} with theme {theme:string} and output format {format:string}"
)]
fn project_config_with_theme_and_format(
    world: &TestWorld,
    file_name: String,
    theme: String,
    format: String,
) -> Result<()> {
    let temp_dir = world
        .temp_dir
        .borrow()
        .as_ref()
        .context("temp_dir should be set")?
        .path()
        .to_path_buf();

    let config_path = temp_dir.join(&file_name);
    let config_content = format!(
        r#"
theme = "{theme}"
output_format = "{format}"
"#
    );
    fs::write(&config_path, config_content)
        .with_context(|| format!("failed to write {file_name}"))?;

    std::env::set_current_dir(&temp_dir).context("failed to change to temp directory")?;

    Ok(())
}

#[given("a project config file {file_name:string} with default targets {targets:string}")]
fn project_config_with_default_targets(
    world: &TestWorld,
    file_name: String,
    targets: String,
) -> Result<()> {
    let temp_dir = world
        .temp_dir
        .borrow()
        .as_ref()
        .context("temp_dir should be set")?
        .path()
        .to_path_buf();

    // Parse comma-separated targets into TOML array format
    let targets_vec: Vec<&str> = targets.split(',').map(str::trim).collect();
    let targets_toml = format!(
        "[{}]",
        targets_vec
            .iter()
            .map(|t| format!("\"{t}\""))
            .collect::<Vec<_>>()
            .join(", ")
    );

    let config_path = temp_dir.join(&file_name);
    let config_content = format!(
        r"
default_targets = {targets_toml}
"
    );
    fs::write(&config_path, config_content)
        .with_context(|| format!("failed to write {file_name}"))?;

    std::env::set_current_dir(&temp_dir).context("failed to change to temp directory")?;

    Ok(())
}

#[given("a custom config file {file_name:string} with theme {theme:string}")]
fn custom_config_with_theme(world: &TestWorld, file_name: String, theme: String) -> Result<()> {
    let temp_dir = world
        .temp_dir
        .borrow()
        .as_ref()
        .context("temp_dir should be set")?
        .path()
        .to_path_buf();

    let config_path = temp_dir.join(&file_name);
    let config_content = format!(
        r#"
theme = "{theme}"
"#
    );
    fs::write(&config_path, config_content)
        .with_context(|| format!("failed to write {file_name}"))?;

    Ok(())
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "BDD step functions must return Result<()>"
)]
#[given("the environment variable {var_name:string} is set to {value:string}")]
fn env_var_is_set(world: &TestWorld, var_name: String, value: String) -> Result<()> {
    // Store the guard so it lives for the scenario
    let _lock = EnvLock::acquire();
    let original = std::env::var_os(&var_name);

    // SAFETY: EnvLock serialises mutations
    unsafe {
        std::env::set_var(&var_name, &value);
    }

    // Track for cleanup
    world
        .env_vars
        .borrow_mut()
        .insert(var_name.clone(), original);

    Ok(())
}

#[given("the environment variable {var_name:string} points to {file_name:string}")]
fn env_var_points_to_file(world: &TestWorld, var_name: String, file_name: String) -> Result<()> {
    let temp_dir = world
        .temp_dir
        .borrow()
        .as_ref()
        .context("temp_dir should be set")?
        .path()
        .to_path_buf();

    let file_path = temp_dir.join(&file_name);

    let _lock = EnvLock::acquire();
    let original = std::env::var_os(&var_name);

    // SAFETY: EnvLock serialises mutations
    unsafe {
        std::env::set_var(&var_name, file_path.as_os_str());
    }

    world
        .env_vars
        .borrow_mut()
        .insert(var_name.clone(), original);

    Ok(())
}

#[then("the theme preference is {expected:string}")]
fn theme_preference_is(world: &TestWorld, expected: String) -> Result<()> {
    let expected_theme = match expected.as_str() {
        "unicode" => ThemePreference::Unicode,
        "ascii" => ThemePreference::Ascii,
        "auto" => ThemePreference::Auto,
        _ => anyhow::bail!("unknown theme preference: {expected}"),
    };

    let actual = world
        .cli
        .with_ref(|cli| cli.theme)
        .flatten()
        .context("CLI theme should be present")?;

    ensure!(
        actual == expected_theme,
        "expected theme {expected}, got {actual:?}"
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
