//! Step definitions for locale resolution scenarios.
//!
//! These steps exercise locale precedence and normalization using the
//! production resolution helpers.

use crate::bdd::fixtures::TestWorld;
use anyhow::{Context, Result, ensure};
use netsuke::cli::Cli;
use netsuke::cli_localization;
use netsuke::locale_resolution::{self, EnvProvider, SystemLocale};
use netsuke::localization::keys;
use ortho_config::{LocalizationArgs, Localizer, MergeComposer, sanitize_value};
use rstest_bdd_macros::{given, then, when};
use serde_json::json;
use std::ffi::OsString;

#[derive(Debug, Default)]
struct StubEnv {
    locale: Option<String>,
}

impl EnvProvider for StubEnv {
    fn var(&self, key: &str) -> Option<String> {
        if key == locale_resolution::NETSUKE_LOCALE_ENV {
            return self.locale.clone();
        }
        None
    }
}

#[derive(Debug, Default)]
struct StubSystemLocale {
    locale: Option<String>,
}

impl SystemLocale for StubSystemLocale {
    fn system_locale(&self) -> Option<String> {
        self.locale.clone()
    }
}

fn build_tokens(args: &str) -> Vec<OsString> {
    let mut tokens = vec![OsString::from("netsuke")];
    let trimmed = args.trim();
    if trimmed.is_empty() {
        return tokens;
    }
    match shlex::split(trimmed) {
        Some(split_args) => tokens.extend(split_args.into_iter().map(OsString::from)),
        None => tokens.extend(trimmed.split_whitespace().map(OsString::from)),
    }
    tokens
}

fn merge_locale_layers(world: &TestWorld) -> Result<Cli> {
    let mut composer = MergeComposer::new();
    let defaults = sanitize_value(&Cli::default())?;
    composer.push_defaults(defaults);

    if let Some(locale) = world.locale_config.get() {
        composer.push_file(json!({ "locale": locale }), None);
    }

    if let Some(locale) = world.locale_env.get() {
        composer.push_environment(json!({ "locale": locale }));
    }

    if let Some(locale) = world.locale_cli_override.get() {
        composer.push_cli(json!({ "locale": locale }));
    }

    Cli::merge_from_layers(composer.layers()).context("merge locale layers")
}

fn record_resolved_locale(world: &TestWorld, resolved: Option<String>) {
    match resolved {
        Some(locale) => world.resolved_locale.set(locale),
        None => world.resolved_locale.clear(),
    }
}

fn which_message(localizer: &dyn Localizer) -> String {
    let mut args = LocalizationArgs::default();
    args.insert("command", "tool".into());
    args.insert("count", 0_i64.into());
    args.insert("preview", "<none>".into());
    localizer.message(keys::STDLIB_WHICH_NOT_FOUND, Some(&args), "not found")
}

#[given("the system locale is {locale:string}")]
fn set_system_locale(world: &TestWorld, locale: &str) {
    world.locale_system.set(locale.to_owned());
}

#[given("the environment locale is {locale:string}")]
fn set_environment_locale(world: &TestWorld, locale: &str) {
    world.locale_env.set(locale.to_owned());
}

#[given("the configuration locale is {locale:string}")]
fn set_configuration_locale(world: &TestWorld, locale: &str) {
    world.locale_config.set(locale.to_owned());
}

#[given("the CLI locale override is {locale:string}")]
fn set_cli_override(world: &TestWorld, locale: &str) {
    world.locale_cli_override.set(locale.to_owned());
}

#[when("the startup locale is resolved for {args:string}")]
fn resolve_startup_locale(world: &TestWorld, args: &str) {
    let env = StubEnv {
        locale: world.locale_env.get(),
    };
    let system = StubSystemLocale {
        locale: world.locale_system.get(),
    };
    let tokens = build_tokens(args);
    let resolved = locale_resolution::resolve_startup_locale(&tokens, &env, &system);
    record_resolved_locale(world, resolved);
}

#[when("the runtime locale is resolved")]
fn resolve_runtime_locale(world: &TestWorld) -> Result<()> {
    let merged = merge_locale_layers(world)?;
    let system = StubSystemLocale {
        locale: world.locale_system.get(),
    };
    let resolved = locale_resolution::resolve_runtime_locale(&merged, &system);
    record_resolved_locale(world, resolved);
    Ok(())
}

#[when("the runtime localizer is built")]
fn build_runtime_localizer(world: &TestWorld) -> Result<()> {
    let merged = merge_locale_layers(world)?;
    let system = StubSystemLocale {
        locale: world.locale_system.get(),
    };
    let resolved = locale_resolution::resolve_runtime_locale(&merged, &system);
    record_resolved_locale(world, resolved.clone());

    let localizer = cli_localization::build_localizer(resolved.as_deref());
    let message = which_message(localizer.as_ref());
    world.locale_message.set(message);
    Ok(())
}

#[then("the resolved locale is {locale:string}")]
fn resolved_locale_is(world: &TestWorld, locale: &str) -> Result<()> {
    let resolved = world.resolved_locale.get();
    ensure!(
        resolved.as_deref() == Some(locale),
        "expected resolved locale '{locale}', got {resolved:?}"
    );
    Ok(())
}

#[then("the localized message contains {fragment:string}")]
fn localized_message_contains(world: &TestWorld, fragment: &str) -> Result<()> {
    let message = world
        .locale_message
        .get()
        .context("expected a localized message to be captured")?;
    ensure!(
        message.contains(fragment),
        "expected localized message to contain '{fragment}', got '{message}'"
    );
    Ok(())
}
