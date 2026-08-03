//! Tests covering localization helpers and fallback behaviour.

use std::sync::{Arc, MutexGuard};

use anyhow::{Context, Result, bail, ensure};
use rstest::rstest;
use test_support::localizer_test_lock;

use fluent_bundle::FluentValue;
use netsuke::cli_localization;
use netsuke::locale_catalogues::SUPPORTED_LOCALES;
use netsuke::localization::{self, LocalizerGuard, keys};
use ortho_config::LocalizationArgs;
use ortho_config::{FluentLocalizer, LanguageIdentifier};
use std::str::FromStr;
use test_support::fluent::normalize_fluent_isolates;

/// Guard pair holding both the test lock and the localizer override.
///
/// The test lock ensures localization tests run serially, and the localizer
/// guard restores the previous localizer when dropped.
///
/// Both fields are underscore-prefixed because they exist only to be dropped:
/// nothing reads them, and the prefix states that without suppressing the
/// `dead_code` lint. `test_support::localizer::EnLocalizer` is named the same
/// way for the same reason.
///
/// Field order is load-bearing: struct fields drop in declaration order, so
/// `_localizer` must precede `_lock`. `LocalizerGuard::drop` writes the global
/// localizer, which is what `_lock` serializes; releasing the lock first would
/// let a waiting test install its own override and capture this test's
/// override as its "previous", so that test would later restore the wrong
/// value.
struct LocalizerTestGuards {
    _localizer: LocalizerGuard,
    _lock: MutexGuard<'static, ()>,
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
        _localizer: guard,
        _lock: lock,
    })
}

fn which_message(command: &str) -> String {
    localization::message(keys::STDLIB_WHICH_NOT_FOUND)
        .with_arg("command", command)
        .with_arg("count", 0)
        .with_arg("preview", "<none>")
        .to_string()
}

/// The number of catalogues this release ships.
///
/// `tests/locale_registry_tests.rs` pins the exact tag set; this count is what
/// lets the sweeps below assert they covered all of it rather than silently
/// iterating a shortened registry.
const EXPECTED_SHIPPED_LOCALE_COUNT: usize = 35;

/// Counts chosen to select every CLDR cardinal category some shipped locale
/// uses.
///
/// `zero` and `two` are Welsh and Arabic; `few` and `many` are Polish, Russian,
/// and Czech among others; `one` and `other` are near-universal. A locale
/// ignores the counts that its plural rules do not distinguish, so one list
/// serves them all.
const PLURAL_PROBE_COUNTS: [i64; 9] = [0, 1, 2, 3, 5, 6, 11, 21, 100];

/// The `(locale, count)` pairs whose idiomatic wording carries no numeral.
///
/// Arabic and Hebrew name small quantities as words rather than digits — "one
/// file", and a dual form for two — and Arabic's `zero` variant reads "no files
/// were processed". Omitting the numeral there is correct translation, not a
/// dropped interpolation, so these are listed rather than excused by a weaker
/// assertion: the sweep below requires the numeral everywhere else, and
/// requires its *absence* here, so a regression in either direction fails.
const NUMERAL_OMITTED_BY_IDIOM: [(&str, i64); 5] =
    [("ar", 0), ("ar", 1), ("ar", 2), ("he", 1), ("he", 2)];

/// Render `key` with a numeric `count`, as Fluent's plural selector requires.
///
/// `LocalizedMessage::with_arg` stringifies its value, and a `FluentValue::String`
/// never matches a plural category — every locale would silently fall to
/// `*[other]`. Passing a `FluentValue::from(i64)` is what actually exercises
/// `intl_pluralrules`.
fn render_with_count(key: &str, count: i64) -> Option<String> {
    let mut args: LocalizationArgs<'_> = LocalizationArgs::new();
    args.insert("count", FluentValue::from(count));
    localization::localizer().lookup(key, Some(&args))
}
/// Every catalogue must parse on its own, with no English underneath it.
///
/// `build_localizer` layers the requested locale over the English source, so a
/// catalogue that fails to parse still renders — in English, with the same
/// arguments interpolated. That is indistinguishable from a working
/// translation at the rendering level, which is why the sweep below cannot
/// catch it. Building each resource directly with the defaults disabled can.
#[test]
fn every_catalogue_parses_without_the_english_fallback() -> Result<()> {
    for entry in SUPPORTED_LOCALES {
        let locale = LanguageIdentifier::from_str(entry.tag())
            .with_context(|| format!("locale {} is not a valid BCP 47 tag", entry.tag()))?;
        let built = FluentLocalizer::builder(locale)
            .with_consumer_resources([entry.resource()])
            .disable_defaults()
            .try_build();
        if let Err(err) = built {
            bail!(
                "locale {} has a catalogue that does not parse: {err}",
                entry.tag()
            );
        }
    }
    Ok(())
}
/// Every registered locale must render a message and interpolate its
/// arguments. This is a rendering sweep, not a parse check: see
/// `every_catalogue_parses_without_the_english_fallback` for the latter.
#[test]
fn every_locale_renders_and_interpolates() -> Result<()> {
    let mut covered = 0usize;
    for entry in SUPPORTED_LOCALES {
        let _guards = localizer_guards(entry.tag())?;
        let message = normalize_fluent_isolates(&which_message("cc"));
        ensure!(
            !message.trim().is_empty(),
            "locale {} rendered an empty message",
            entry.tag()
        );
        ensure!(
            message.contains("cc") && message.contains('0'),
            "locale {} did not interpolate its arguments, got: {message}",
            entry.tag()
        );
        ensure!(
            !message.contains(keys::STDLIB_WHICH_NOT_FOUND),
            "locale {} rendered the key identifier instead of a message: {message}",
            entry.tag()
        );
        covered += 1;
    }
    ensure!(
        covered == EXPECTED_SHIPPED_LOCALE_COUNT,
        "the sweep covered {covered} locales, expected {EXPECTED_SHIPPED_LOCALE_COUNT}"
    );
    Ok(())
}

/// Non-Latin catalogues must reach the terminal with their own script intact.
#[rstest]
#[case("ja", '\u{3040}', '\u{30FF}')]
#[case("ko", '\u{AC00}', '\u{D7A3}')]
#[case("ru", '\u{0400}', '\u{04FF}')]
#[case("el", '\u{0370}', '\u{03FF}')]
#[case("th", '\u{0E00}', '\u{0E7F}')]
#[case("hi", '\u{0900}', '\u{097F}')]
#[case("zh-Hans", '\u{4E00}', '\u{9FFF}')]
fn non_latin_locales_render_their_own_script(
    #[case] locale: &str,
    #[case] first: char,
    #[case] last: char,
) -> Result<()> {
    let _guards = localizer_guards(locale)?;

    let message = localization::message(keys::MANIFEST_PARSE).to_string();
    ensure!(
        message.chars().any(|ch| (first..=last).contains(&ch)),
        "expected {locale} to render characters in {first:?}..={last:?}, got: {message}"
    );
    Ok(())
}

/// Right-to-left locales must render right-to-left text, and a message that
/// opens with a Latin token must still carry the mark that pins the
/// paragraph's direction.
#[rstest]
#[case("ar", '\u{0600}', '\u{06FF}')]
#[case("fa", '\u{0600}', '\u{06FF}')]
#[case("he", '\u{0590}', '\u{05FF}')]
fn rtl_locales_render_and_keep_direction_marks(
    #[case] locale: &str,
    #[case] first: char,
    #[case] last: char,
) -> Result<()> {
    let _guards = localizer_guards(locale)?;

    let message = localization::message(keys::MANIFEST_PARSE).to_string();
    ensure!(
        message.chars().any(|ch| (first..=last).contains(&ch)),
        "expected {locale} to render its own script, got: {message}"
    );

    let label = localization::message(keys::MANIFEST_YAML_LABEL).to_string();
    ensure!(
        label.starts_with('\u{200F}'),
        "expected {locale} to keep the right-to-left mark on a Latin-initial \
         message, got: {label:?}"
    );
    Ok(())
}

#[rstest]
#[case("es-ES", "no encontrado")]
// Icelandic ships no catalogue, so the English source copy renders.
#[case("is-IS", "not found")]
// A tag that will not parse at all takes the same path.
#[case("not a locale", "not found")]
#[case("", "not found")]
// A region with no catalogue of its own reaches its language's copy.
#[case("de-AT", "nicht gefunden")]
// A Latin American region reaches es-419 rather than Spain's catalogue.
#[case("es-MX", "no se encontró")]
// Script and region variants stay apart at run time, not just in resolution.
#[case("zh-TW", "找不到")]
#[case("zh-CN", "未找到")]
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
#[case("en-US", "Stage timing summary:", "Stage 1/6", "Total pipeline time:")]
#[case(
    "es-ES",
    "Resumen de tiempos por etapa:",
    "Etapa 1/6",
    "Tiempo total de la canalización:"
)]
fn timing_summary_messages_resolve(
    #[case] locale: &str,
    #[case] expected_header: &str,
    #[case] expected_stage_label: &str,
    #[case] expected_total_prefix: &str,
) -> Result<()> {
    let _guards = localizer_guards(locale)?;

    let header = localization::message(keys::STATUS_TIMING_SUMMARY_HEADER).to_string();
    let label = localization::message(keys::STATUS_STAGE_LABEL)
        .with_arg("current", 1)
        .with_arg("total", 6)
        .with_arg(
            "description",
            localization::message(keys::STATUS_STAGE_MANIFEST_INGESTION),
        )
        .to_string();
    let stage_line = localization::message(keys::STATUS_TIMING_STAGE_LINE)
        .with_arg("label", &label)
        .with_arg("duration", "12ms")
        .to_string();
    let total_line = localization::message(keys::STATUS_TIMING_TOTAL_LINE)
        .with_arg("duration", "50ms")
        .to_string();

    let normalized_header = normalize_fluent_isolates(&header);
    let normalized_label = normalize_fluent_isolates(&label);
    let normalized_stage_line = normalize_fluent_isolates(&stage_line);
    let normalized_total_line = normalize_fluent_isolates(&total_line);

    ensure!(
        normalized_header.contains(expected_header),
        "expected timing header for locale {locale} to contain {expected_header:?}, got: {header}"
    );
    ensure!(
        normalized_label.contains(expected_stage_label),
        "expected timing label for locale {locale} to contain {expected_stage_label:?}, got: {label}"
    );
    ensure!(
        normalized_stage_line.starts_with("- "),
        "expected timing stage line for locale {locale} to preserve bullet prefix, got: {stage_line}"
    );
    ensure!(
        normalized_stage_line.contains(&normalized_label),
        "expected timing stage line for locale {locale} to include stage label {label:?}, got: {stage_line}"
    );
    ensure!(
        normalized_stage_line.ends_with(": 12ms"),
        "expected timing stage line for locale {locale} to end with ': 12ms', got: {stage_line}"
    );
    ensure!(
        normalized_total_line.contains(expected_total_prefix),
        "expected timing total line for locale {locale} to contain {expected_total_prefix:?}, got: {total_line}"
    );
    Ok(())
}

/// Plural selection must work at runtime, for every shipped locale.
///
/// `tests/locale_catalogue_tests.rs` checks that each catalogue *declares* the
/// CLDR categories its language needs. That is a structural check on the FTL
/// text; it cannot tell whether Fluent actually selects those variants when
/// given a number. This renders `example.files_processed` through the live
/// localizer for a spread of counts, so a catalogue whose variants are declared
/// but unreachable fails here.
#[test]
fn plural_selection_renders_for_every_locale_and_count() -> Result<()> {
    let mut covered = 0usize;
    for entry in SUPPORTED_LOCALES {
        let _guards = localizer_guards(entry.tag())?;
        for count in PLURAL_PROBE_COUNTS {
            let rendered =
                render_with_count(keys::EXAMPLE_FILES_PROCESSED, count).with_context(|| {
                    format!(
                        "locale {} returned no message for count {count}",
                        entry.tag()
                    )
                })?;
            let message = normalize_fluent_isolates(&rendered);
            ensure!(
                !message.trim().is_empty(),
                "locale {} rendered empty for count {count}",
                entry.tag()
            );
            ensure!(
                !message.contains(keys::EXAMPLE_FILES_PROCESSED),
                "locale {} fell back to the key identifier for count {count}: {message}",
                entry.tag()
            );
            let numeral_expected = !NUMERAL_OMITTED_BY_IDIOM.contains(&(entry.tag(), count));
            ensure!(
                message.contains(&count.to_string()) == numeral_expected,
                "locale {} count {count}: expected the numeral present={numeral_expected}, got {message}",
                entry.tag()
            );
        }
        covered += 1;
    }
    ensure!(
        covered == EXPECTED_SHIPPED_LOCALE_COUNT,
        "plural sweep covered {covered} locales, expected {EXPECTED_SHIPPED_LOCALE_COUNT}"
    );
    Ok(())
}

/// A numeric argument must actually reach the plural selector.
///
/// This is the guard on the helper above: if `render_with_count` ever passed a
/// string, every locale would render its `*[other]` variant and the sweep would
/// still pass. A language whose `one` and `other` wordings differ proves the
/// selector ran.
#[test]
fn a_numeric_count_selects_a_different_variant_from_the_default() -> Result<()> {
    let _guards = localizer_guards("en-US")?;

    let singular = render_with_count(keys::EXAMPLE_FILES_PROCESSED, 1)
        .context("en-US must render for count 1")?;
    let plural = render_with_count(keys::EXAMPLE_FILES_PROCESSED, 2)
        .context("en-US must render for count 2")?;

    ensure!(
        singular != plural,
        "count 1 and count 2 must select different variants, both gave {singular}"
    );
    Ok(())
}

/// A stringified count must NOT select a category, which is why the helper
/// exists. Pinning this stops someone "simplifying" the helper back to
/// `with_arg` and silently disabling every plural assertion above.
#[test]
fn a_stringified_count_falls_through_to_the_default_variant() -> Result<()> {
    let _guards = localizer_guards("en-US")?;

    let mut args: LocalizationArgs<'_> = LocalizationArgs::new();
    args.insert("count", FluentValue::from("1"));
    let as_string = localization::localizer()
        .lookup(keys::EXAMPLE_FILES_PROCESSED, Some(&args))
        .context("en-US must render for a string count")?;
    let as_number = render_with_count(keys::EXAMPLE_FILES_PROCESSED, 1)
        .context("en-US must render for a numeric count")?;

    ensure!(
        as_string != as_number,
        "a string count must not select the `one` variant; both gave {as_string}"
    );
    Ok(())
}
