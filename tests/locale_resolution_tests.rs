//! Unit tests for locale resolution helpers.

use anyhow::{Result, ensure};
use netsuke::cli::Cli;
use netsuke::locale_resolution::{
    EnvProvider, NETSUKE_LOCALE_ENV, SystemLocale, normalize_locale_tag, resolve_runtime_locale,
    resolve_startup_locale,
};
use rstest::rstest;
use std::ffi::OsString;

#[derive(Debug, Default)]
struct StubEnv {
    locale: Option<&'static str>,
}

impl EnvProvider for StubEnv {
    fn var(&self, key: &str) -> Option<String> {
        if key == NETSUKE_LOCALE_ENV {
            return self.locale.map(ToString::to_string);
        }
        None
    }
}

#[derive(Debug, Default)]
struct StubSystemLocale {
    locale: Option<&'static str>,
}

impl SystemLocale for StubSystemLocale {
    fn system_locale(&self) -> Option<String> {
        self.locale.map(ToString::to_string)
    }
}

fn build_args(args: &[&str]) -> Vec<OsString> {
    args.iter().map(|arg| OsString::from(*arg)).collect()
}

#[rstest]
#[case("en_US.UTF-8", Some("en-US"))]
#[case("es_ES", Some("es-ES"))]
#[case("pt_BR@latin", Some("pt-BR"))]
#[case("  en-GB  ", Some("en-GB"))]
#[case("C", None)]
#[case("", None)]
fn normalize_locale_tag_handles_common_formats(
    #[case] raw: &str,
    #[case] expected: Option<&str>,
) -> Result<()> {
    let normalized = normalize_locale_tag(raw);
    ensure!(
        normalized.as_deref() == expected,
        "expected {:?} to normalize to {:?}, got {:?}",
        raw,
        expected,
        normalized
    );
    Ok(())
}

#[rstest]
fn resolve_startup_locale_prefers_cli_over_env_and_system() -> Result<()> {
    let args = build_args(&["netsuke", "--locale", "es-ES"]);
    let env = StubEnv {
        locale: Some("fr-FR"),
    };
    let system = StubSystemLocale {
        locale: Some("en_US"),
    };
    let resolved = resolve_startup_locale(&args, &env, &system);
    ensure!(
        resolved.as_deref() == Some("es-ES"),
        "expected CLI locale to win, got {resolved:?}"
    );
    Ok(())
}

#[rstest]
fn resolve_startup_locale_uses_env_then_system() -> Result<()> {
    let args = build_args(&["netsuke"]);
    let env = StubEnv {
        locale: Some("fr-FR"),
    };
    let system = StubSystemLocale {
        locale: Some("es_ES.UTF-8"),
    };
    let resolved = resolve_startup_locale(&args, &env, &system);
    ensure!(
        resolved.as_deref() == Some("fr-FR"),
        "expected env locale to win, got {resolved:?}"
    );

    let env_fallback = StubEnv { locale: None };
    let resolved_fallback = resolve_startup_locale(&args, &env_fallback, &system);
    ensure!(
        resolved_fallback.as_deref() == Some("es-ES"),
        "expected system locale fallback, got {resolved_fallback:?}"
    );
    Ok(())
}

#[rstest]
fn resolve_runtime_locale_uses_merged_config_then_system() -> Result<()> {
    let cli = Cli {
        locale: Some("es-ES".to_owned()),
        ..Cli::default()
    };
    let system = StubSystemLocale {
        locale: Some("en_US"),
    };
    let resolved = resolve_runtime_locale(&cli, &system);
    ensure!(
        resolved.as_deref() == Some("es-ES"),
        "expected merged locale to win, got {resolved:?}"
    );

    let cli_invalid = Cli {
        locale: Some("bad locale".to_owned()),
        ..Cli::default()
    };
    let resolved_invalid = resolve_runtime_locale(&cli_invalid, &system);
    ensure!(
        resolved_invalid.as_deref() == Some("en-US"),
        "expected system locale fallback, got {resolved_invalid:?}"
    );
    Ok(())
}
