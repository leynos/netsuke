//! Step definitions for configuration discovery scenarios.

use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use crate::bdd::helpers::env_mutation::mutate_env_var;
use crate::bdd::types::{EnvVarKey, EnvVarValue, FileName, NamesList};
use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs_utf8::Dir};
use netsuke::cli::Cli;
use netsuke::cli::config::EmojiPolicy;
use rstest_bdd_macros::{given, then, when};
use std::ffi::OsString;
use tempfile::tempdir;

use super::cli::apply_cli_tokens;

#[given("a temporary workspace")]
fn a_temporary_workspace(world: &TestWorld) -> Result<()> {
    let temp = tempdir().context("failed to create temporary workspace")?;
    *world.temp_dir.borrow_mut() = Some(temp);
    Ok(())
}

/// Write `content` to `file_name` inside `world`'s temp directory.
fn write_config_file(world: &TestWorld, file_name: &str, content: &str) -> Result<()> {
    let temp_dir = world
        .temp_dir
        .borrow()
        .as_ref()
        .context("temp_dir should be set")?
        .path()
        .to_path_buf();

    let temp_dir_utf8 = temp_dir
        .to_str()
        .context("temp dir path must be valid UTF-8")?;
    let dir = Dir::open_ambient_dir(temp_dir_utf8, ambient_authority())
        .with_context(|| format!("open workspace {temp_dir_utf8} to write {file_name}"))?;
    dir.write(file_name, content.as_bytes())
        .with_context(|| format!("failed to write {file_name} in {temp_dir_utf8}"))?;

    Ok(())
}

#[given("a project config file {file_name:string} with emoji {emoji:string} and jobs {jobs}")]
fn project_config_with_emoji_and_jobs(
    world: &TestWorld,
    file_name: FileName,
    emoji: EmojiPolicy,
    jobs: u32,
) -> Result<()> {
    let content = format!(
        r#"
emoji = "{emoji}"
jobs = {jobs}
"#
    );
    write_config_file(world, file_name.as_str(), &content)
}

#[given("a malformed project config file {file_name:string}")]
fn malformed_project_config(world: &TestWorld, file_name: FileName) -> Result<()> {
    write_config_file(world, file_name.as_str(), "emoji = \"always\n")
}

/// Returns the TOML snippet for a config file that sets only `emoji`.
fn emoji_config_content(emoji: EmojiPolicy) -> String {
    format!("\nemoji = \"{emoji}\"\n")
}

#[given("a project config file {file_name:string} with emoji {emoji:string}")]
fn project_config_with_emoji(
    world: &TestWorld,
    file_name: FileName,
    emoji: EmojiPolicy,
) -> Result<()> {
    write_config_file(world, file_name.as_str(), &emoji_config_content(emoji))
}

#[given("a project config file {file_name:string} with emoji {emoji:string} and JSON {json}")]
fn project_config_with_emoji_and_json(
    world: &TestWorld,
    file_name: FileName,
    emoji: EmojiPolicy,
    json: bool,
) -> Result<()> {
    let content = format!(
        r#"
emoji = "{emoji}"
json = {json}
"#
    );
    write_config_file(world, file_name.as_str(), &content)
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
    write_config_file(world, file_name.as_str(), &content)
}

#[given("a custom config file {file_name:string} with emoji {emoji:string}")]
fn custom_config_with_emoji(
    world: &TestWorld,
    file_name: FileName,
    emoji: EmojiPolicy,
) -> Result<()> {
    write_config_file(world, file_name.as_str(), &emoji_config_content(emoji))
}

#[when("the CLI is parsed with the workspace config file {file_name:string}")]
fn parse_cli_with_workspace_config(world: &TestWorld, file_name: FileName) -> Result<()> {
    let config_path = world
        .temp_dir
        .borrow()
        .as_ref()
        .context("temp_dir should be set")?
        .path()
        .join(file_name.as_str());
    apply_cli_tokens(
        world,
        vec![
            OsString::from("netsuke"),
            OsString::from("--config"),
            config_path.into_os_string(),
        ],
    );
    Ok(())
}

#[given("the environment variable {var_name:string} is set to {value:string}")]
fn env_var_is_set(world: &TestWorld, var_name: EnvVarKey, value: EnvVarValue) -> Result<()> {
    mutate_env_var(world, var_name, Some(value.as_str()))
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
    let file_path_str = file_path
        .to_str()
        .context("file path must be valid UTF-8")?;

    mutate_env_var(world, var_name, Some(file_path_str))
}

/// Reads an optional field from the resolved CLI struct stored in `world`.
///
/// Returns an error if the field is absent.
fn read_cli_option<T, F>(world: &TestWorld, field_name: &str, extract: F) -> Result<T>
where
    F: FnOnce(&Cli) -> Option<T>,
{
    world
        .cli
        .with_ref(|cli| extract(cli))
        .flatten()
        .with_context(|| format!("CLI {field_name} should be present"))
}

#[then("the jobs setting is {expected}")]
fn jobs_setting_is(world: &TestWorld, expected: u32) -> Result<()> {
    let actual = read_cli_option(world, "jobs", |cli| cli.jobs)?;
    ensure!(
        u32::try_from(actual)? == expected,
        "expected jobs {expected}, got {actual}"
    );
    Ok(())
}
