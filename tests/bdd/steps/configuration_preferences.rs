//! Step definitions for canonical layered CLI configuration preferences.

use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use crate::bdd::helpers::env_mutation::mutate_env_var;
use crate::bdd::helpers::parse_store::store_parse_outcome;
use crate::bdd::helpers::tokens::build_tokens;
use crate::bdd::types::EnvVarKey;
use anyhow::{Context, Result, bail, ensure};
use netsuke::cli::{AccessibilityPolicy, Cli, ColourPolicy, Commands, EmojiPolicy, ProgressPolicy};
use netsuke::cli_localization;
use rstest_bdd_macros::{given, then, when};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use test_support::display_error_chain;

const CONFIG_ENV_VAR: &str = "NETSUKE_CONFIG";
const LOCALE_ENV_VAR: &str = "NETSUKE_LOCALE";

fn workspace_path(world: &TestWorld) -> Result<PathBuf> {
    world
        .temp_dir
        .borrow()
        .as_ref()
        .map(|dir| dir.path().to_path_buf())
        .context("temp dir has not been initialised for configuration steps")
}

fn write_config(world: &TestWorld, contents: &str) -> Result<()> {
    let path = workspace_path(world)?.join("netsuke.toml");
    fs::write(&path, contents).with_context(|| format!("write {}", path.display()))?;
    let config_path = path
        .to_str()
        .context("configuration path must be valid UTF-8")?;
    mutate_env_var(world, EnvVarKey::from(CONFIG_ENV_VAR), Some(config_path))
}

fn merge_cli(world: &TestWorld, args: &str) {
    let tokens = build_tokens(args);
    let localizer = Arc::from(cli_localization::build_localizer(None));
    let outcome = netsuke::cli::parse_with_localizer_from(tokens, &localizer)
        .and_then(|(cli, matches)| {
            netsuke::cli::merge_with_config(&cli, &matches).map_err(|err| {
                clap::Error::raw(
                    clap::error::ErrorKind::InvalidValue,
                    display_error_chain(err.as_ref()),
                )
            })
        })
        .map(Cli::with_default_command)
        .map_err(|err| err.to_string());
    store_parse_outcome(&world.cli, &world.cli_error, outcome);
}

fn parse_value<T: clap::ValueEnum>(value: &str, label: &str) -> Result<T> {
    T::from_str(value, true).map_err(|err| anyhow::anyhow!("invalid {label} '{value}': {err}"))
}

fn assert_merged_field<T>(
    world: &TestWorld,
    expected: T,
    label: &str,
    extract: impl FnOnce(&Cli) -> T,
) -> Result<()>
where
    T: Copy + PartialEq + std::fmt::Debug,
{
    let value = world
        .cli
        .with_ref(extract)
        .context("expected merged CLI to be available")?;
    ensure!(
        value == expected,
        "expected merged {label} to be {expected:?}, got {value:?}",
    );
    Ok(())
}

#[given("the Netsuke config file sets build targets to {target:string}")]
fn config_sets_build_targets(world: &TestWorld, target: &str) -> Result<()> {
    write_config(world, &format!("[cmds.build]\ntargets = [\"{target}\"]\n"))
}

#[given("the Netsuke config file sets locale to {locale:string}")]
fn config_sets_locale(world: &TestWorld, locale: &str) -> Result<()> {
    write_config(world, &format!("locale = \"{locale}\"\n"))
}

#[given("the NETSUKE_LOCALE environment variable is {locale:string}")]
fn set_environment_locale_override(world: &TestWorld, locale: &str) -> Result<()> {
    mutate_env_var(world, EnvVarKey::from(LOCALE_ENV_VAR), Some(locale))
}

#[given("the Netsuke config file sets color to {policy:string}")]
fn config_sets_color(world: &TestWorld, policy: &str) -> Result<()> {
    let _: ColourPolicy = parse_value(policy, "color policy")?;
    write_config(world, &format!("color = \"{policy}\"\n"))
}

#[given("the Netsuke config file sets emoji to {policy:string}")]
fn config_sets_emoji(world: &TestWorld, policy: &str) -> Result<()> {
    let _: EmojiPolicy = parse_value(policy, "emoji policy")?;
    write_config(world, &format!("emoji = \"{policy}\"\n"))
}

#[given("the Netsuke config file sets progress to {policy:string}")]
fn config_sets_progress(world: &TestWorld, policy: &str) -> Result<()> {
    let _: ProgressPolicy = parse_value(policy, "progress policy")?;
    write_config(world, &format!("progress = \"{policy}\"\n"))
}

#[given("the Netsuke config file sets accessibility to {policy:string}")]
fn config_sets_accessibility(world: &TestWorld, policy: &str) -> Result<()> {
    let _: AccessibilityPolicy = parse_value(policy, "accessibility policy")?;
    write_config(world, &format!("accessibility = \"{policy}\"\n"))
}

#[given("the Netsuke config file disables no-input")]
fn config_disables_no_input(world: &TestWorld) -> Result<()> {
    write_config(world, "no_input = false\n")
}

#[given("the {name:string} environment variable is {value:string}")]
fn set_environment_preference(world: &TestWorld, name: &str, value: &str) -> Result<()> {
    mutate_env_var(world, EnvVarKey::from(name), Some(value))
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macro generates Result wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[when("the CLI is parsed and merged with {args:string}")]
fn parse_and_merge_cli(world: &TestWorld, args: &str) -> Result<()> {
    merge_cli(world, args);
    Ok(())
}

#[then("the merged CLI uses build target {target:string}")]
fn merged_cli_uses_build_target(world: &TestWorld, target: &str) -> Result<()> {
    let command = world
        .cli
        .with_ref(|cli| cli.command.clone())
        .context("expected merged CLI to be available")?
        .context("expected merged CLI command to be set")?;
    match command {
        Commands::Build(args) => ensure!(
            args.targets.first().map(String::as_str) == Some(target),
            "expected first merged build target '{target}', got {:?}",
            args.targets,
        ),
        other => bail!("expected merged build command, got {other:?}"),
    }
    Ok(())
}

#[then("the merged locale is {locale:string}")]
fn merged_locale_is(world: &TestWorld, locale: &str) -> Result<()> {
    let actual = world
        .cli
        .with_ref(|cli| cli.locale.clone())
        .context("expected merged CLI to be available")?;
    ensure!(
        actual.as_deref() == Some(locale),
        "expected locale '{locale}'"
    );
    Ok(())
}

#[then("verbose mode is enabled in the merged CLI")]
fn merged_verbose_enabled(world: &TestWorld) -> Result<()> {
    assert_merged_field(world, true, "verbose mode", |cli| cli.verbose)
}

#[then("the merged color policy is {expected:string}")]
fn merged_color_policy(world: &TestWorld, expected: &str) -> Result<()> {
    assert_merged_field(
        world,
        parse_value(expected, "color policy")?,
        "color policy",
        |cli| cli.color,
    )
}

#[then("the merged emoji policy is {expected:string}")]
fn merged_emoji_policy(world: &TestWorld, expected: &str) -> Result<()> {
    assert_merged_field(
        world,
        parse_value(expected, "emoji policy")?,
        "emoji policy",
        |cli| cli.emoji,
    )
}

#[then("the merged progress policy is {expected:string}")]
fn merged_progress_policy(world: &TestWorld, expected: &str) -> Result<()> {
    assert_merged_field(
        world,
        parse_value(expected, "progress policy")?,
        "progress policy",
        |cli| cli.progress,
    )
}

#[then("the merged accessibility policy is {expected:string}")]
fn merged_accessibility_policy(world: &TestWorld, expected: &str) -> Result<()> {
    assert_merged_field(
        world,
        parse_value(expected, "accessibility policy")?,
        "accessibility policy",
        |cli| cli.accessibility,
    )
}

#[then("the merge error should contain {fragment:string}")]
fn merge_error_contains(world: &TestWorld, fragment: &str) -> Result<()> {
    let error = world
        .cli_error
        .get()
        .context("expected a merge error to be captured")?;
    ensure!(
        error.contains(fragment),
        "expected merge error to contain '{fragment}', got '{error}'",
    );
    Ok(())
}
