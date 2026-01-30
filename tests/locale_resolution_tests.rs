//! Unit tests for locale resolution helpers.

use anyhow::{Result, ensure};
use netsuke::cli::Cli;
use netsuke::locale_resolution::{
    normalize_locale_tag, resolve_runtime_locale, resolve_startup_locale,
};
use rstest::rstest;
use std::ffi::OsString;
use test_support::locale_stubs::{StubEnv, StubSystemLocale};

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
    let env = StubEnv::with_locale("fr-FR");
    let system = StubSystemLocale::with_locale("en_US");
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
    let env = StubEnv::with_locale("fr-FR");
    let system = StubSystemLocale::with_locale("es_ES.UTF-8");
    let resolved = resolve_startup_locale(&args, &env, &system);
    ensure!(
        resolved.as_deref() == Some("fr-FR"),
        "expected env locale to win, got {resolved:?}"
    );

    let env_fallback = StubEnv::default();
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
    let system = StubSystemLocale::with_locale("en_US");
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

#[rstest]
fn resolve_runtime_locale_returns_none_when_no_valid_locale() -> Result<()> {
    let cli_invalid = Cli {
        locale: Some("bad locale".to_owned()),
        ..Cli::default()
    };
    let system_invalid = StubSystemLocale::with_locale("also bad");
    let resolved_both_invalid = resolve_runtime_locale(&cli_invalid, &system_invalid);
    ensure!(
        resolved_both_invalid.is_none(),
        "expected None when both merged and system locales are invalid, got {resolved_both_invalid:?}"
    );

    let cli_none = Cli {
        locale: None,
        ..Cli::default()
    };
    let system_none = StubSystemLocale::default();
    let resolved_both_none = resolve_runtime_locale(&cli_none, &system_none);
    ensure!(
        resolved_both_none.is_none(),
        "expected None when no usable locale is available, got {resolved_both_none:?}"
    );
    Ok(())
}
