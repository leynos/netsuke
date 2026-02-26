//! Tests covering localization helpers and fallback behaviour.

use std::sync::{Arc, MutexGuard};

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use test_support::localizer_test_lock;

use netsuke::cli_localization;
use netsuke::localization::{self, LocalizerGuard, keys};
use test_support::fluent::normalize_fluent_isolates;

/// Guard pair holding both the test lock and the localizer override.
///
/// The test lock ensures localization tests run serially, and the localizer
/// guard restores the previous localizer when dropped.
struct LocalizerTestGuards {
    #[expect(dead_code, reason = "Held for lifetime, not accessed directly")]
    lock: MutexGuard<'static, ()>,
    #[expect(dead_code, reason = "Held for lifetime, not accessed directly")]
    localizer: LocalizerGuard,
}

/// Create localizer guards for a given locale.
///
/// This helper acquires the test lock and sets up the localizer for the
/// specified locale, returning guards that restore state when dropped.
fn localizer_guards(locale: &str) -> Result<LocalizerTestGuards> {
    let lock = localizer_test_lock()
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("localizer test lock poisoned")?;
    let localizer = cli_localization::build_localizer(Some(locale));
    let guard = localization::set_localizer_for_tests(Arc::from(localizer));
    Ok(LocalizerTestGuards {
        lock,
        localizer: guard,
    })
}

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
    let _guards = localizer_guards(locale)?;

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
    let _guards = localizer_guards(locale)?;

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
    let _guards = localizer_guards(locale)?;

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
    let _guards = localizer_guards("en-US")?;

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

#[rstest]
#[case("en-US", "Stage 2/6", "pending")]
#[case("es-ES", "Etapa 2/6", "pendiente")]
fn progress_stage_messages_resolve(
    #[case] locale: &str,
    #[case] expected_label: &str,
    #[case] expected_state: &str,
) -> Result<()> {
    let _guards = localizer_guards(locale)?;

    let label = localization::message(keys::STATUS_STAGE_LABEL)
        .with_arg("current", 2)
        .with_arg("total", 6)
        .with_arg(
            "description",
            localization::message(keys::STATUS_STAGE_TEMPLATE_EXPANSION),
        )
        .to_string();
    let summary = localization::message(keys::STATUS_STAGE_SUMMARY)
        .with_arg("state", localization::message(keys::STATUS_STATE_PENDING))
        .with_arg("label", &label)
        .to_string();
    let normalized_label = normalize_fluent_isolates(&label);
    let normalized_summary = normalize_fluent_isolates(&summary);

    ensure!(
        normalized_label.contains(expected_label),
        "expected stage label for locale {locale} to contain {expected_label:?}, got: {label}"
    );
    ensure!(
        normalized_summary.contains(expected_state),
        "expected summary state for locale {locale} to contain {expected_state:?}, got: {summary}"
    );
    Ok(())
}

#[rstest]
#[case("en-US", "Task 2/6", "cc -c src/main.c")]
#[case("es-ES", "Tarea 2/6", "cc -c src/main.c")]
fn progress_task_messages_resolve(
    #[case] locale: &str,
    #[case] expected_label: &str,
    #[case] expected_description: &str,
) -> Result<()> {
    let _guards = localizer_guards(locale)?;

    let task_label = localization::message(keys::STATUS_TASK_PROGRESS_LABEL)
        .with_arg("current", 2)
        .with_arg("total", 6)
        .to_string();
    let task_update = localization::message(keys::STATUS_TASK_PROGRESS_UPDATE)
        .with_arg("task", &task_label)
        .with_arg("description", "cc -c src/main.c")
        .to_string();
    let normalized_label = normalize_fluent_isolates(&task_label);
    let normalized_update = normalize_fluent_isolates(&task_update);

    ensure!(
        normalized_label.contains(expected_label),
        "expected task label for locale {locale} to contain {expected_label:?}, got: {task_label}"
    );
    ensure!(
        normalized_update.contains(expected_description),
        "expected task update for locale {locale} to contain {expected_description:?}, got: {task_update}"
    );
    Ok(())
}

#[rstest]
#[case("en-US", "Stage timing summary:", "Total pipeline time:")]
#[case(
    "es-ES",
    "Resumen de tiempos por etapa:",
    "Tiempo total de la canalización:"
)]
fn timing_summary_messages_resolve(
    #[case] locale: &str,
    #[case] expected_header: &str,
    #[case] expected_total_prefix: &str,
) -> Result<()> {
    let _guards = localizer_guards(locale)?;

    let header = localization::message(keys::STATUS_TIMING_SUMMARY_HEADER).to_string();
    let stage_line = localization::message(keys::STATUS_TIMING_STAGE_LINE)
        .with_arg("label", "Stage 1/6: Reading manifest file")
        .with_arg("duration", "12ms")
        .to_string();
    let total_line = localization::message(keys::STATUS_TIMING_TOTAL_LINE)
        .with_arg("duration", "50ms")
        .to_string();

    let normalized_header = normalize_fluent_isolates(&header);
    let normalized_stage_line = normalize_fluent_isolates(&stage_line);
    let normalized_total_line = normalize_fluent_isolates(&total_line);

    ensure!(
        normalized_header.contains(expected_header),
        "expected timing header for locale {locale} to contain {expected_header:?}, got: {header}"
    );
    ensure!(
        normalized_stage_line.contains("12ms"),
        "expected timing stage line for locale {locale} to include duration, got: {stage_line}"
    );
    ensure!(
        normalized_total_line.contains(expected_total_prefix),
        "expected timing total line for locale {locale} to contain {expected_total_prefix:?}, got: {total_line}"
    );
    Ok(())
}
