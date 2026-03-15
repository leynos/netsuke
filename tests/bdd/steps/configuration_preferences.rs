//! Step definitions for layered CLI configuration preferences.

use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use crate::bdd::helpers::parse_store::store_parse_outcome;
use crate::bdd::helpers::tokens::build_tokens;
use anyhow::{Context, Result, anyhow, bail, ensure};
use netsuke::cli::{Cli, Commands, Theme};
use netsuke::cli_localization;
use netsuke::output_prefs;
use rstest_bdd_macros::{given, then, when};
use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use test_support::EnvVarGuard;
use test_support::display_error_chain;
use test_support::env_lock::EnvLock;

const CONFIG_ENV_VAR: &str = "NETSUKE_CONFIG_PATH";
const LOCALE_ENV_VAR: &str = "NETSUKE_LOCALE";

fn workspace_path(world: &TestWorld) -> Result<PathBuf> {
    let temp = world.temp_dir.borrow();
    let dir = temp
        .as_ref()
        .context("temp dir has not been initialised for configuration steps")?;
    Ok(dir.path().to_path_buf())
}

fn ensure_env_lock(world: &TestWorld) {
    if world.env_lock.borrow().is_none() {
        *world.env_lock.borrow_mut() = Some(EnvLock::acquire());
    }
}

fn write_config(world: &TestWorld, contents: &str) -> Result<()> {
    ensure_env_lock(world);
    let workspace = workspace_path(world)?;
    let path = workspace.join("netsuke.toml");
    fs::write(&path, contents).with_context(|| format!("write {}", path.display()))?;
    // SAFETY: `EnvLock` is held in `world.env_lock` for the lifetime of the scenario.
    let guard = EnvVarGuard::set(CONFIG_ENV_VAR, path.as_os_str());
    let previous = guard.original();
    world.track_env_var(CONFIG_ENV_VAR.to_owned(), previous);
    Ok(())
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

#[given("the Netsuke config file sets build targets to {target:string}")]
fn config_sets_build_targets(world: &TestWorld, target: &str) -> Result<()> {
    write_config(world, &format!("[cmds.build]\ntargets = [\"{target}\"]\n"))
}

#[given("the Netsuke config file sets locale to {locale:string}")]
fn config_sets_locale(world: &TestWorld, locale: &str) -> Result<()> {
    write_config(world, &format!("locale = \"{locale}\"\n"))
}

#[given("the NETSUKE_LOCALE environment variable is {locale:string}")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macro generates Result wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
fn set_environment_locale_override(world: &TestWorld, locale: &str) -> Result<()> {
    ensure_env_lock(world);
    // SAFETY: `EnvLock` is held in `world.env_lock` for the lifetime of the scenario.
    let guard = EnvVarGuard::set(LOCALE_ENV_VAR, OsStr::new(locale));
    let previous = guard.original();
    world.track_env_var(LOCALE_ENV_VAR.to_owned(), previous);
    Ok(())
}

#[given("the Netsuke config file sets output format to {format:string}")]
fn config_sets_output_format(world: &TestWorld, format: &str) -> Result<()> {
    write_config(world, &format!("output_format = \"{format}\"\n"))
}

#[given("the Netsuke config file sets no_emoji to true")]
fn config_sets_no_emoji(world: &TestWorld) -> Result<()> {
    write_config(world, "no_emoji = true\n")
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

#[when("merged output preferences are resolved")]
fn resolve_merged_output_prefs(world: &TestWorld) -> Result<()> {
    let prefs = world
        .cli
        .with_ref(|cli| output_prefs::resolve(cli.no_emoji_override()))
        .ok_or_else(|| anyhow!("expected merged CLI before resolving output prefs"))?;
    world.output_prefs.set(prefs);
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
    let merged_locale = world
        .cli
        .with_ref(|cli| cli.locale.clone())
        .context("expected merged CLI to be available")?;
    ensure!(
        merged_locale.as_deref() == Some(locale),
        "expected merged locale '{locale}', got {merged_locale:?}",
    );
    Ok(())
}

#[then("verbose mode is enabled in the merged CLI")]
fn merged_verbose_enabled(world: &TestWorld) -> Result<()> {
    let verbose = world
        .cli
        .with_ref(|cli| cli.verbose)
        .context("expected merged CLI to be available")?;
    ensure!(verbose, "expected merged verbose mode to be enabled");
    Ok(())
}

#[then("the merged theme is ascii")]
fn merged_theme_is_ascii(world: &TestWorld) -> Result<()> {
    let theme = world
        .cli
        .with_ref(|cli| cli.theme)
        .context("expected merged CLI to be available")?;
    ensure!(
        theme == Some(Theme::Ascii),
        "expected merged theme to be ASCII, got {theme:?}",
    );
    Ok(())
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
