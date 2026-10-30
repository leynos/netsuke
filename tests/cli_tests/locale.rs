//! Locale and JSON hint CLI tests.

use anyhow::{Context, Result, ensure};
use clap::error::ErrorKind;
use rstest::rstest;
use test_support::fluent::normalize_fluent_isolates;
use test_support::tracing_capture::with_test_subscriber;
use tracing_subscriber::filter::LevelFilter;

use crate::cli_tests::helpers::os_args;
use netsuke::cli::{json_hint_from_args, locale_hint_from_args};
use netsuke::cli_localization;
use std::sync::Arc;

#[rstest]
fn locale_hint_from_args_accepts_space_form() -> Result<()> {
    let args = os_args(&["netsuke", "--locale", "es-ES"]);
    let hint = locale_hint_from_args(&args);
    ensure!(
        hint.as_deref() == Some("es-ES"),
        "expected Some(\"es-ES\"), got: {hint:?}"
    );
    Ok(())
}

#[rstest]
fn locale_hint_from_args_accepts_equals_form() -> Result<()> {
    let args = os_args(&["netsuke", "--locale=es-ES"]);
    let hint = locale_hint_from_args(&args);
    ensure!(
        hint.as_deref() == Some("es-ES"),
        "expected Some(\"es-ES\"), got: {hint:?}"
    );
    Ok(())
}

#[rstest]
fn locale_hint_from_args_trailing_locale_flag_yields_none() -> Result<()> {
    let args = os_args(&["netsuke", "--locale"]);
    let hint = locale_hint_from_args(&args);
    ensure!(
        hint.is_none(),
        "expected None for trailing --locale without value, got: {hint:?}"
    );
    Ok(())
}

#[rstest]
fn locale_hint_from_args_ignores_args_after_double_dash() -> Result<()> {
    let args = os_args(&["netsuke", "--verbose", "--", "--locale", "es-ES"]);
    let hint = locale_hint_from_args(&args);
    ensure!(
        hint.is_none(),
        "expected None when --locale appears after \"--\", got: {hint:?}"
    );
    Ok(())
}

#[rstest]
fn locale_hint_from_args_uses_last_locale_flag() -> Result<()> {
    let args = os_args(&["netsuke", "--locale", "es-ES", "--locale", "en-US"]);
    let hint = locale_hint_from_args(&args);
    ensure!(
        hint.as_deref() == Some("en-US"),
        "expected last --locale to win (\"en-US\"), got: {hint:?}"
    );
    Ok(())
}

#[rstest]
fn json_hint_from_args_accepts_bare_flag() -> Result<()> {
    let args = os_args(&["netsuke", "--json"]);
    let hint = json_hint_from_args(&args);
    ensure!(hint == Some(true), "expected Some(true), got: {hint:?}");
    Ok(())
}

#[rstest]
fn json_hint_from_args_ignores_equals_form() -> Result<()> {
    let args = os_args(&["netsuke", "--json=false"]);
    let hint = json_hint_from_args(&args);
    ensure!(hint.is_none(), "expected None, got: {hint:?}");
    Ok(())
}

#[rstest]
fn json_hint_from_args_ignores_args_after_double_dash() -> Result<()> {
    let args = os_args(&["netsuke", "--", "--json"]);
    let hint = json_hint_from_args(&args);
    ensure!(
        hint.is_none(),
        "expected None when --json appears after \"--\", got: {hint:?}"
    );
    Ok(())
}

#[rstest]
fn json_hint_from_args_detects_bare_flag_after_ignored_equals_form() -> Result<()> {
    let args = os_args(&["netsuke", "--json=false", "--json"]);
    let hint = json_hint_from_args(&args);
    ensure!(hint == Some(true), "expected Some(true), got: {hint:?}");
    Ok(())
}

#[rstest]
fn cli_localises_invalid_subcommand_in_spanish() -> Result<()> {
    let localizer = Arc::from(cli_localization::build_localizer(Some("es-ES")));
    let err = netsuke::cli::parse_with_localizer_from(
        ["netsuke", "--locale", "es-ES", "unknown"],
        &localizer,
    )
    .err()
    .context("parser should reject invalid subcommand")?;
    ensure!(
        err.to_string().contains("Subcomando desconocido"),
        "expected Spanish localization, got: {err}",
    );
    Ok(())
}

/// Parse `argv` with a localizer for `locale` and return the rejection.
fn parse_rejection(argv: &[&str], locale: &str) -> Result<clap::Error> {
    let localizer = Arc::from(cli_localization::build_localizer(Some(locale)));
    netsuke::cli::parse_with_localizer_from(argv.iter().copied(), &localizer)
        .err()
        .with_context(|| format!("parser should reject {argv:?}"))
}

/// A `ValueValidation` error from a custom parser must carry enough clap
/// context for `clap-error-value-validation` to format. Without `InvalidArg`
/// and `InvalidValue` the localizer falls back and ships clap's raw text.
#[rstest]
#[case::color("--color", "loud", "--color <POLICY>")]
#[case::emoji("--emoji", "sometimes", "--emoji <POLICY>")]
#[case::progress("--progress", "paused", "--progress <POLICY>")]
#[case::accessibility("--accessibility", "yes", "--accessibility <POLICY>")]
#[case::jobs("--jobs", "notanumber", "--jobs <N>")]
#[case::locale("--locale", "nope", "--locale <LOCALE>")]
#[case::allow_scheme("--fetch-allow-scheme", "1bad", "--fetch-allow-scheme <SCHEME>")]
#[case::allow_host("--fetch-allow-host", "bad host", "--fetch-allow-host <HOST>")]
#[case::block_host("--fetch-block-host", "bad host", "--fetch-block-host <HOST>")]
fn value_validation_errors_render_localized_message(
    #[case] flag: &str,
    #[case] value: &str,
    #[case] expected_argument: &str,
) -> Result<()> {
    let err = parse_rejection(&["netsuke", flag, value], "en-US")?;

    ensure!(
        err.kind() == ErrorKind::ValueValidation,
        "expected ValueValidation for {flag}, got {:?}",
        err.kind()
    );
    let rendered = normalize_fluent_isolates(&err.to_string());
    let expected = format!("Validation failed for {expected_argument}: {value}");
    ensure!(
        rendered.contains(&expected),
        "expected {expected:?} in rendered error, got: {rendered}"
    );
    Ok(())
}

/// The same key must resolve through a non-English catalogue, proving the
/// context reaches the localizer rather than falling back to English.
#[rstest]
fn value_validation_errors_render_spanish_template() -> Result<()> {
    let err = parse_rejection(&["netsuke", "--color", "loud"], "es-ES")?;
    let rendered = normalize_fluent_isolates(&err.to_string());

    ensure!(
        rendered.contains("Validación fallida para --color <POLICY>: loud"),
        "expected Spanish value-validation template, got: {rendered}"
    );
    Ok(())
}

/// A resolvable catalogue message must not also emit the missing-translation
/// warning that the context-free path produced.
#[rstest]
fn value_validation_errors_do_not_warn_about_missing_translation() -> Result<()> {
    let (rejection, events) = with_test_subscriber(LevelFilter::WARN, |captured| {
        let rejection = parse_rejection(&["netsuke", "--color", "loud"], "en-US");
        (rejection, captured.snapshot())
    });
    let err = rejection?;
    ensure!(
        normalize_fluent_isolates(&err.to_string()).contains("Validation failed for"),
        "expected the localized template, got: {err}"
    );
    ensure!(
        events
            .iter()
            .all(|event| !event.contains("missing translation")),
        "resolvable key should not warn about a missing translation: {events:?}"
    );
    Ok(())
}

/// Non-UTF-8 values fail before the validation closure runs and must render
/// the placeholder-free `clap-error-invalid-utf8` catalogue message.
#[cfg(unix)]
#[rstest]
fn non_utf8_value_validation_renders_localized_utf8_message() -> Result<()> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let (result, events) = with_test_subscriber(LevelFilter::WARN, |captured| {
        let localizer = Arc::from(cli_localization::build_localizer(Some("en-US")));
        let outcome = netsuke::cli::parse_with_localizer_from(
            [
                OsString::from("netsuke"),
                OsString::from("--locale"),
                OsString::from_vec(b"\xff".to_vec()),
            ],
            &localizer,
        );
        (outcome, captured.snapshot())
    });
    let err = result.expect_err("non-UTF-8 value should fail during CLI parsing");

    ensure!(
        err.kind() == ErrorKind::InvalidUtf8,
        "expected InvalidUtf8, got {:?}",
        err.kind()
    );
    ensure!(
        normalize_fluent_isolates(&err.to_string())
            .contains("Invalid UTF-8 was detected in one or more arguments"),
        "expected the localized UTF-8 diagnostic, got: {err}"
    );
    ensure!(
        events
            .iter()
            .all(|event| !event.contains("missing translation")),
        "the invalid-UTF-8 key should resolve without a warning: {events:?}"
    );
    Ok(())
}
