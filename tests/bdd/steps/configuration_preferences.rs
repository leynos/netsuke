//! Step definitions for layered CLI configuration preferences.

use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use crate::bdd::helpers::parse_store::store_parse_outcome;
use crate::bdd::helpers::tokens::build_tokens;
use anyhow::{Context, Result, anyhow, bail, ensure};
use clap::ValueEnum as _;
use netsuke::cli::{Cli, ColourPolicy, Commands, OutputFormat, SpinnerMode, Theme};
use netsuke::cli_localization;
use netsuke::output_prefs;
use rstest_bdd_macros::{given, then, when};
use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
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
    let previous = std::env::var_os(CONFIG_ENV_VAR);
    // SAFETY: `EnvLock` is held in `world.env_lock` for the lifetime of the scenario.
    unsafe { std::env::set_var(CONFIG_ENV_VAR, path.as_os_str()) };
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

/// Convert a clap `ValueEnum` to its canonical name string.
///
/// # Panics
///
/// Panics if the enum variant does not have a possible value (which should
/// never happen for well-formed `ValueEnum` implementations).
fn enum_name<T: clap::ValueEnum>(value: &T) -> String {
    value
        .to_possible_value()
        .unwrap_or_else(|| panic!("all ValueEnum variants must have a possible value"))
        .get_name()
        .to_owned()
}

/// Write a theme value to the config file.
fn write_theme_config(world: &TestWorld, theme: Theme) -> Result<()> {
    write_config(world, &format!("theme = \"{}\"\n", enum_name(&theme)))
}

/// Write a colour policy value to the config file.
fn write_colour_policy_config(world: &TestWorld, policy: ColourPolicy) -> Result<()> {
    write_config(
        world,
        &format!("colour_policy = \"{}\"\n", enum_name(&policy)),
    )
}

/// Write a spinner mode value to the config file.
fn write_spinner_mode_config(world: &TestWorld, mode: SpinnerMode) -> Result<()> {
    write_config(world, &format!("spinner_mode = \"{}\"\n", enum_name(&mode)))
}

/// Write an output format value to the config file.
fn write_output_format_config(world: &TestWorld, format: OutputFormat) -> Result<()> {
    write_config(
        world,
        &format!("output_format = \"{}\"\n", enum_name(&format)),
    )
}

/// Set the `NETSUKE_THEME` environment variable.
fn set_env_theme(world: &TestWorld, theme: Theme) {
    ensure_env_lock(world);
    let previous = std::env::var_os("NETSUKE_THEME");
    let value = enum_name(&theme);
    // SAFETY: `EnvLock` is held in `world.env_lock` for the lifetime of the scenario.
    unsafe { std::env::set_var("NETSUKE_THEME", OsStr::new(&value)) };
    world.track_env_var("NETSUKE_THEME".to_owned(), previous);
}

/// Set the `NETSUKE_COLOUR_POLICY` environment variable.
fn set_env_colour_policy(world: &TestWorld, policy: ColourPolicy) {
    ensure_env_lock(world);
    let previous = std::env::var_os("NETSUKE_COLOUR_POLICY");
    let value = enum_name(&policy);
    // SAFETY: `EnvLock` is held in `world.env_lock` for the lifetime of the scenario.
    unsafe { std::env::set_var("NETSUKE_COLOUR_POLICY", OsStr::new(&value)) };
    world.track_env_var("NETSUKE_COLOUR_POLICY".to_owned(), previous);
}

/// Set the `NETSUKE_SPINNER_MODE` environment variable.
fn set_env_spinner_mode(world: &TestWorld, mode: SpinnerMode) {
    ensure_env_lock(world);
    let previous = std::env::var_os("NETSUKE_SPINNER_MODE");
    let value = enum_name(&mode);
    // SAFETY: `EnvLock` is held in `world.env_lock` for the lifetime of the scenario.
    unsafe { std::env::set_var("NETSUKE_SPINNER_MODE", OsStr::new(&value)) };
    world.track_env_var("NETSUKE_SPINNER_MODE".to_owned(), previous);
}

/// Assert a merged CLI field value matches the expected value.
fn assert_merged_field<T, F>(world: &TestWorld, extract: F, expected: T, label: &str) -> Result<()>
where
    T: Copy + PartialEq + std::fmt::Debug,
    F: FnOnce(&Cli) -> Option<T>,
{
    let value = world
        .cli
        .with_ref(|cli| extract(cli))
        .context("expected merged CLI to be available")?;
    ensure!(
        value == Some(expected),
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
#[expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macro generates Result wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
fn set_environment_locale_override(world: &TestWorld, locale: &str) -> Result<()> {
    ensure_env_lock(world);
    let previous = std::env::var_os(LOCALE_ENV_VAR);
    // SAFETY: `EnvLock` is held in `world.env_lock` for the lifetime of the scenario.
    unsafe { std::env::set_var(LOCALE_ENV_VAR, OsStr::new(locale)) };
    world.track_env_var(LOCALE_ENV_VAR.to_owned(), previous);
    Ok(())
}

#[given("the Netsuke config file sets output format to {format:string}")]
fn config_sets_output_format(world: &TestWorld, format: &str) -> Result<()> {
    let typed = OutputFormat::from_str(format, true)
        .map_err(|err| anyhow!("invalid output format '{format}': {err}"))?;
    write_output_format_config(world, typed)
}

#[given("the Netsuke config file sets no_emoji to true")]
fn config_sets_no_emoji(world: &TestWorld) -> Result<()> {
    write_config(world, "no_emoji = true\n")
}

#[given("the Netsuke config file sets theme to {theme:string}")]
fn config_sets_theme(world: &TestWorld, theme: &str) -> Result<()> {
    let typed =
        Theme::from_str(theme, true).map_err(|err| anyhow!("invalid theme '{theme}': {err}"))?;
    write_theme_config(world, typed)
}

#[given("the Netsuke config file sets colour policy to {policy:string}")]
fn config_sets_colour_policy(world: &TestWorld, policy: &str) -> Result<()> {
    let typed = ColourPolicy::from_str(policy, true)
        .map_err(|err| anyhow!("invalid colour policy '{policy}': {err}"))?;
    write_colour_policy_config(world, typed)
}

#[given("the Netsuke config file sets spinner mode to {mode:string}")]
fn config_sets_spinner_mode(world: &TestWorld, mode: &str) -> Result<()> {
    let typed = SpinnerMode::from_str(mode, true)
        .map_err(|err| anyhow!("invalid spinner mode '{mode}': {err}"))?;
    write_spinner_mode_config(world, typed)
}

#[given("the NETSUKE_THEME environment variable is {theme:string}")]
fn set_environment_theme_override(world: &TestWorld, theme: &str) -> Result<()> {
    let typed =
        Theme::from_str(theme, true).map_err(|err| anyhow!("invalid theme '{theme}': {err}"))?;
    set_env_theme(world, typed);
    Ok(())
}

#[given("the NETSUKE_COLOUR_POLICY environment variable is {policy:string}")]
fn set_environment_colour_policy_override(world: &TestWorld, policy: &str) -> Result<()> {
    let typed = ColourPolicy::from_str(policy, true)
        .map_err(|err| anyhow!("invalid colour policy '{policy}': {err}"))?;
    set_env_colour_policy(world, typed);
    Ok(())
}

#[given("the NETSUKE_SPINNER_MODE environment variable is {mode:string}")]
fn set_environment_spinner_mode_override(world: &TestWorld, mode: &str) -> Result<()> {
    let typed = SpinnerMode::from_str(mode, true)
        .map_err(|err| anyhow!("invalid spinner mode '{mode}': {err}"))?;
    set_env_spinner_mode(world, typed);
    Ok(())
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
    assert_merged_field(world, |cli| cli.theme, Theme::Ascii, "theme")
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

#[then("the merged theme is unicode")]
fn merged_theme_is_unicode(world: &TestWorld) -> Result<()> {
    assert_merged_field(world, |cli| cli.theme, Theme::Unicode, "theme")
}

#[then("the merged colour policy is always")]
fn merged_colour_policy_is_always(world: &TestWorld) -> Result<()> {
    assert_merged_field(
        world,
        |cli| cli.colour_policy,
        ColourPolicy::Always,
        "colour policy",
    )
}

#[then("the merged spinner mode is enabled")]
fn merged_spinner_mode_is_enabled(world: &TestWorld) -> Result<()> {
    assert_merged_field(
        world,
        |cli| cli.spinner_mode,
        SpinnerMode::Enabled,
        "spinner mode",
    )
}
