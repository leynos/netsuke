//! Locale-specific CLI tests.

use anyhow::{Context, Result, ensure};
use rstest::rstest;

use crate::helpers::os_args;
use netsuke::cli::locale_hint_from_args;
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
