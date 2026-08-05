//! Runtime plural-selection tests.
//!
//! Split from `localization_tests.rs` to keep both files within the
//! repository's 400-line limit. These cover what Fluent *selects* at run time;
//! `tests/locale_catalogue_tests.rs` covers what each catalogue *declares*,
//! which is a different question and deliberately kept separate.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use fluent_bundle::FluentValue;
use netsuke::locale_catalogues::SUPPORTED_LOCALES;
use netsuke::localization::{self, keys};
use ortho_config::LocalizationArgs;
use rstest::rstest;
use test_support::fluent::normalize_fluent_isolates;
use test_support::localizer::locale_localizer;

/// The number of catalogues this release ships.
///
/// `tests/locale_registry_tests.rs` pins the exact tag set; this count lets the
/// sweeps below assert they covered all of it rather than silently iterating a
/// shortened registry.
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
/// were processed". Hindi overrides count zero with an exact `[0]` variant
/// reading "no files were processed", since its CLDR `one` category would
/// otherwise render "0 फ़ाइल". Omitting the numeral there is correct
/// translation, not a dropped interpolation, so these are listed rather than
/// excused by a weaker assertion: the sweep below requires the numeral
/// everywhere else, and requires its *absence* here, so a regression in either
/// direction fails.
const NUMERAL_OMITTED_BY_IDIOM: [(&str, i64); 6] = [
    ("ar", 0),
    ("ar", 1),
    ("ar", 2),
    ("he", 1),
    ("he", 2),
    ("hi", 0),
];

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
        let _guards = locale_localizer(entry.tag());
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
    let _guards = locale_localizer("en-US");

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
    let _guards = locale_localizer("en-US");

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

/// One declared variant of a `select` expression.
struct DeclaredVariant {
    category: String,
    /// The variant's literal text, still carrying `{ $count }`.
    template: String,
    /// Whether this is the `*` default, chosen when no category matches.
    is_default: bool,
}

/// The variants a catalogue's `key` declares.
///
/// Exact numeric variants such as `[0]` are declared branches too: Fluent
/// selects an exact match ahead of any plural category, so leaving them out
/// would make the oracle below reject the very rendering the catalogue asks
/// for at that count.
fn declared_variants(resource: &str, key: &str) -> Vec<DeclaredVariant> {
    let mut lines = resource.lines().map(str::trim);
    let opened = lines
        .find_map(|trimmed| trimmed.split_once('=').filter(|(id, _)| id.trim() == key))
        .is_some_and(|(_, value)| value.trim().ends_with("->"));
    if !opened {
        return Vec::new();
    }
    lines
        .take_while(|trimmed| *trimmed != "}")
        .filter_map(|trimmed| {
            let is_default = trimmed.starts_with('*');
            let (name, text) = trimmed
                .trim_start_matches('*')
                .strip_prefix('[')?
                .split_once(']')?;
            Some(DeclaredVariant {
                category: name.to_owned(),
                template: text.trim().to_owned(),
                is_default,
            })
        })
        .collect()
}

/// Which declared categories could have produced `rendered` for `count`.
///
/// More than one qualifies when a locale words two categories identically —
/// Hungarian and Turkish keep the noun singular after any numeral — so the
/// result is a set rather than a single answer.
fn matching_categories(variants: &[DeclaredVariant], rendered: &str, count: i64) -> Vec<String> {
    variants
        .iter()
        .filter(|variant| {
            let expected = variant.template.replace("{ $count }", &count.to_string());
            normalize_fluent_isolates(&expected) == rendered
        })
        .map(|variant| variant.category.clone())
        .collect()
}

/// Every locale must select a declared branch, and at least one that is not
/// the default.
///
/// The rendering sweep above cannot tell selection from fallback: a locale
/// that always resolved to `*[other]` would still render non-empty text
/// containing the numeral. This checks the rendered string against the
/// catalogue's own variant templates, so the branch Fluent actually chose is
/// identified rather than assumed.
#[test]
fn every_locale_selects_a_declared_plural_branch() -> Result<()> {
    let mut covered = 0usize;
    for entry in SUPPORTED_LOCALES {
        let variants = declared_variants(entry.resource(), keys::EXAMPLE_FILES_PROCESSED);
        ensure!(
            !variants.is_empty(),
            "locale {} declares no plural variants",
            entry.tag()
        );
        // Taken from this key's own variants: the first `*[` in the whole
        // catalogue belongs to whichever select comes first, which need not be
        // this one.
        let default_category = variants
            .iter()
            .find(|variant| variant.is_default)
            .map_or_else(|| "other".to_owned(), |variant| variant.category.clone());

        let _guards = locale_localizer(entry.tag());
        let mut selected: BTreeSet<String> = BTreeSet::new();
        for count in PLURAL_PROBE_COUNTS {
            let rendered = render_with_count(keys::EXAMPLE_FILES_PROCESSED, count)
                .with_context(|| format!("locale {} rendered nothing", entry.tag()))?;
            let normalized = normalize_fluent_isolates(&rendered);
            let matched = matching_categories(&variants, &normalized, count);
            ensure!(
                !matched.is_empty(),
                "locale {} count {count} rendered {normalized:?}, which matches no declared variant",
                entry.tag()
            );
            selected.extend(matched);
        }

        // A locale declaring only a default has nothing to select between.
        if variants.len() > 1 {
            ensure!(
                selected
                    .iter()
                    .any(|category| *category != default_category),
                "locale {} only ever selected its default `{default_category}` branch across {PLURAL_PROBE_COUNTS:?}",
                entry.tag()
            );
        }
        covered += 1;
    }
    ensure!(
        covered == EXPECTED_SHIPPED_LOCALE_COUNT,
        "the oracle covered {covered} locales, expected {EXPECTED_SHIPPED_LOCALE_COUNT}"
    );
    Ok(())
}

/// The example plural messages must resolve and interpolate their count.
///
/// These pass the count through `LocalizedMessage::with_arg`, which stringifies
/// it, so they exercise the default variant only. That is deliberate: it is the
/// path most call sites take. `every_locale_selects_a_declared_plural_branch`
/// covers numeric selection.
#[rstest]
#[case("en-US", "Processed", "files.")]
#[case("es-ES", "procesaron", "archivos.")]
fn example_files_processed_message_resolves(
    #[case] locale: &str,
    #[case] expected_verb: &str,
    #[case] expected_noun: &str,
) -> Result<()> {
    let _guards = locale_localizer(locale);

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
    let _guards = locale_localizer(locale);

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
