//! Tests covering localization helpers and fallback behaviour.

use std::sync::Arc;

use anyhow::{Result, ensure};
use rstest::rstest;
use test_support::localizer_test_lock;

use netsuke::cli_localization;
use netsuke::localization::{self, keys};

fn which_message(command: &str) -> String {
    localization::message(keys::STDLIB_WHICH_NOT_FOUND)
        .with_arg("command", command)
        .with_arg("count", 0)
        .with_arg("preview", "<none>")
        .to_string()
}

#[rstest]
#[case("es-ES", "no encontrado")]
#[case("fr-FR", "not found")]
fn localisation_resolves_expected_message(
    #[case] locale: &str,
    #[case] expected_substring: &str,
) -> Result<()> {
    let _lock = localizer_test_lock().expect("localizer test lock poisoned");
    let localizer = cli_localization::build_localizer(Some(locale));
    let _guard = localization::set_localizer_for_tests(Arc::from(localizer));

    let message = which_message("tool");
    ensure!(
        message.contains(expected_substring),
        "expected message to contain {expected_substring:?} for locale {locale}, got: {message}"
    );
    Ok(())
}

/// Verify that the example plural form messages are resolvable and interpolate
/// the count variable. Note: CLDR plural selection requires numeric `FluentValue`
/// types, but the current API passes strings, so only the default `[other]`
/// variant is selected. These tests verify the messages resolve and interpolate
/// correctly regardless of which variant is chosen.
#[rstest]
#[case("en-US", "Processed", "files.")]
#[case("es-ES", "procesaron", "archivos.")]
fn example_files_processed_message_resolves(
    #[case] locale: &str,
    #[case] expected_verb: &str,
    #[case] expected_noun: &str,
) -> Result<()> {
    let _lock = localizer_test_lock().expect("localizer test lock poisoned");
    let localizer = cli_localization::build_localizer(Some(locale));
    let _guard = localization::set_localizer_for_tests(Arc::from(localizer));

    let message = localization::message(keys::EXAMPLE_FILES_PROCESSED)
        .with_arg("count", 5)
        .to_string();

    ensure!(
        message.contains(expected_verb),
        "expected message for locale {locale} to contain {expected_verb:?}, got: {message}"
    );
    ensure!(
        message.contains(expected_noun),
        "expected message for locale {locale} to contain {expected_noun:?}, got: {message}"
    );
    // Verify the count variable was interpolated (appears somewhere in the message)
    ensure!(
        message.contains('5'),
        "expected count variable to be interpolated, got: {message}"
    );
    Ok(())
}

/// Verify that the example `errors_found` message resolves and interpolates correctly.
#[rstest]
#[case("en-US", "errors found.")]
#[case("es-ES", "encontraron")]
fn example_errors_found_message_resolves(
    #[case] locale: &str,
    #[case] expected_substring: &str,
) -> Result<()> {
    let _lock = localizer_test_lock().expect("localizer test lock poisoned");
    let localizer = cli_localization::build_localizer(Some(locale));
    let _guard = localization::set_localizer_for_tests(Arc::from(localizer));

    let message = localization::message(keys::EXAMPLE_ERRORS_FOUND)
        .with_arg("count", 3)
        .to_string();

    ensure!(
        message.contains(expected_substring),
        "expected message for locale {locale} to contain {expected_substring:?}, got: {message}"
    );
    // Verify the count variable was interpolated
    ensure!(
        message.contains('3'),
        "expected count variable to be interpolated, got: {message}"
    );
    Ok(())
}

#[rstest]
fn variable_interpolation_works_correctly() -> Result<()> {
    let _lock = localizer_test_lock().expect("localizer test lock poisoned");
    let localizer = cli_localization::build_localizer(Some("en-US"));
    let _guard = localization::set_localizer_for_tests(Arc::from(localizer));

    let message = localization::message(keys::STDLIB_FETCH_URL_INVALID)
        .with_arg("url", "https://example.com")
        .with_arg("details", "connection refused")
        .to_string();

    ensure!(
        message.contains("example.com"),
        "URL variable should be interpolated, got: {message}"
    );
    ensure!(
        message.contains("connection refused"),
        "details variable should be interpolated, got: {message}"
    );
    Ok(())
}
