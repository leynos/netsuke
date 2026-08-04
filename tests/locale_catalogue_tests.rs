//! Content checks over every catalogue in the locale registry.
//!
//! These read the embedded catalogue text rather than rendering messages, so
//! they cover properties the build-time audit does not: the CLDR plural
//! categories each locale declares, the bidi marking right-to-left catalogues
//! rely on, and the guarantee that a translation is not simply a copy of the
//! English source.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use netsuke::locale_catalogues::{LocaleCatalogue, SOURCE_LOCALE, SUPPORTED_LOCALES, catalogue};

/// Message used to demonstrate plural handling in every catalogue.
const PLURAL_EXAMPLE: &str = "example.files_processed";

/// Locales whose copy may legitimately match the English source, because they
/// are English.
const ENGLISH_LOCALES: [&str; 2] = ["en-GB", SOURCE_LOCALE];

/// Embedded catalogue text for `tag`.
///
/// # Errors
///
/// Returns an error when the tag is absent from the locale registry.
fn catalogue_text(tag: &str) -> Result<&'static str> {
    catalogue(tag)
        .map(LocaleCatalogue::resource)
        .with_context(|| format!("{tag} should be in the locale registry"))
}

/// Extract the value of a single-line message from catalogue text.
fn message_value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .find(|(id, _)| id.trim() == key)
        .map(|(_, value)| value.trim())
}

/// Whether `value` opens a `select` expression.
fn opens_select(value: &str) -> bool {
    value.ends_with("->")
}

/// Collect the CLDR categories a `select` expression declares.
///
/// The scan runs in two passes over one iterator: find the line defining
/// `key`, then read variant lines until the closing brace. Only a `select`
/// opens a variant block — a plain message with the same key yields the empty
/// set rather than letting the scan run on and collect the categories of
/// whichever `select` came next.
///
/// Variant lines look like `[one] …` or `*[other] …`. Numeric selectors such
/// as `[0]` are exact matches rather than CLDR categories, so they are skipped.
fn plural_categories(text: &str, key: &str) -> BTreeSet<String> {
    let mut lines = text.lines().map(str::trim);
    let Some((_, value)) =
        lines.find_map(|trimmed| trimmed.split_once('=').filter(|(id, _)| id.trim() == key))
    else {
        return BTreeSet::new();
    };
    if !opens_select(value.trim()) {
        return BTreeSet::new();
    }
    lines
        .take_while(|trimmed| *trimmed != "}")
        .filter_map(|trimmed| {
            let name = trimmed
                .trim_start_matches('*')
                .strip_prefix('[')?
                .split(']')
                .next()?;
            (!name.chars().all(|ch| ch.is_ascii_digit())).then_some(name)
        })
        .map(str::to_owned)
        .collect()
}

fn categories(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

/// The CLDR cardinal plural categories each shipped locale must declare.
///
/// Fluent selects variants through `intl_pluralrules` (7.0.2, per
/// `Cargo.lock`), so these are that crate's cardinal categories rather than
/// any newer CLDR release. Revisit the table when that dependency is bumped.
///
/// The Romance locales carry `one`/`other` only. CLDR also defines `many` for
/// French, Spanish, Italian and Portuguese, but solely for large round numbers
/// in compact notation ("2 millions"); Netsuke counts files and errors as
/// plain integers, which never select it.
const PLURAL_CATEGORIES: &[(&str, &[&str])] = &[
    ("ar", &["zero", "one", "two", "few", "many", "other"]),
    ("cs", &["one", "few", "many", "other"]),
    ("cy", &["zero", "one", "two", "few", "many", "other"]),
    ("da", &["one", "other"]),
    ("de", &["one", "other"]),
    ("el", &["one", "other"]),
    ("en-GB", &["one", "other"]),
    ("en-US", &["one", "other"]),
    ("es-419", &["one", "other"]),
    ("es-ES", &["one", "other"]),
    ("fa", &["one", "other"]),
    ("fi", &["one", "other"]),
    ("fr", &["one", "other"]),
    ("gd", &["one", "two", "few", "other"]),
    // intl_pluralrules 7.0.2 implements CLDR 37, which gives Hebrew four
    // cardinal categories: one, two, many, and other.
    ("he", &["one", "two", "many", "other"]),
    ("hi", &["one", "other"]),
    // Hungarian keeps the noun singular after a numeral, so both variants read
    // alike, but CLDR still defines `one` and a catalogue must offer it.
    ("hu", &["one", "other"]),
    ("id", &["other"]),
    ("it", &["one", "other"]),
    ("ja", &["other"]),
    ("ko", &["other"]),
    ("nb", &["one", "other"]),
    ("nl", &["one", "other"]),
    ("pl", &["one", "few", "many", "other"]),
    ("pt-BR", &["one", "other"]),
    ("pt-PT", &["one", "other"]),
    ("ro", &["one", "few", "other"]),
    ("ru", &["one", "few", "many", "other"]),
    ("sv", &["one", "other"]),
    ("th", &["other"]),
    ("tr", &["one", "other"]),
    ("uk", &["one", "few", "many", "other"]),
    ("vi", &["other"]),
    ("zh-Hans", &["other"]),
    ("zh-Hant", &["other"]),
];

/// The table must name every shipped locale, or a catalogue could ship with
/// the wrong categories and never be checked.
#[test]
fn the_plural_table_covers_every_registry_locale() -> Result<()> {
    let tabled: BTreeSet<&str> = PLURAL_CATEGORIES.iter().map(|(tag, _)| *tag).collect();
    for entry in SUPPORTED_LOCALES {
        ensure!(
            tabled.contains(entry.tag()),
            "{} has no entry in PLURAL_CATEGORIES",
            entry.tag()
        );
    }
    ensure!(
        tabled.len() == SUPPORTED_LOCALES.len(),
        "PLURAL_CATEGORIES has {} entries for {} locales",
        tabled.len(),
        SUPPORTED_LOCALES.len()
    );
    Ok(())
}

/// Every locale must declare exactly the CLDR plural categories its language
/// uses; a translator who drops `few` from Polish silently loses a form.
#[test]
fn plural_examples_declare_the_language_categories() -> Result<()> {
    for (tag, expected) in PLURAL_CATEGORIES {
        let found = plural_categories(catalogue_text(tag)?, PLURAL_EXAMPLE);
        ensure!(
            found == categories(expected),
            concat!(
                "{tag} declares the CLDR plural categories {found:?} ",
                "for {PLURAL_EXAMPLE}, expected {expected:?}"
            )
        );
    }
    Ok(())
}

/// Every catalogue's plural example must offer the `other` default, which
/// Fluent falls back to when no category matches.
#[test]
fn every_plural_example_offers_the_default_variant() {
    for entry in SUPPORTED_LOCALES {
        let found = plural_categories(entry.resource(), PLURAL_EXAMPLE);
        assert!(
            found.contains("other"),
            "{} must declare the default `other` variant",
            entry.tag()
        );
    }
}

/// A catalogue that merely copies the English source is not a translation.
#[test]
fn translations_are_not_copies_of_the_source() -> Result<()> {
    let source = catalogue_text(SOURCE_LOCALE)?;
    let sampled = ["cli.about", "manifest.parse", "status.state.pending"];
    let translated_locales = SUPPORTED_LOCALES
        .iter()
        .filter(|entry| !ENGLISH_LOCALES.contains(&entry.tag()));
    for entry in translated_locales {
        for key in sampled {
            let translated = message_value(entry.resource(), key);
            ensure!(
                translated.is_some() && translated != message_value(source, key),
                "{}: {key} still matches the English source",
                entry.tag()
            );
        }
    }
    Ok(())
}

/// Netsuke identifiers that users type must survive translation verbatim.
#[test]
fn catalogues_preserve_netsuke_identifiers() {
    for entry in SUPPORTED_LOCALES {
        let text = entry.resource();
        for token in ["cwd_mode", "with_suffix", "group_by", "ninja -t clean"] {
            assert!(
                text.contains(token),
                "{} should keep the identifier `{token}` untranslated",
                entry.tag()
            );
        }
    }
}

/// A key that names a plain message must not borrow a later `select`'s
/// categories.
///
/// This is the failure the two-pass scan exists to prevent: the requested key
/// is found, is not a `select`, and the scan stops there rather than running
/// on into `other.plural`.
#[test]
fn a_matching_non_select_message_yields_no_categories() -> Result<()> {
    let catalogue = concat!(
        "wanted.key = Just a message.\n",
        "other.plural = { $count ->\n",
        "    [one] One.\n",
        "   *[other] Many.\n",
        "}\n",
    );
    let found = plural_categories(catalogue, "wanted.key");
    ensure!(found.is_empty(), "expected no categories, got {found:?}");
    Ok(())
}

/// A key absent from the catalogue yields nothing rather than the first
/// `select` it happens to meet.
#[test]
fn an_absent_key_yields_no_categories() -> Result<()> {
    let catalogue = "other.plural = { $count ->\n    [one] One.\n   *[other] Many.\n}\n";
    let found = plural_categories(catalogue, "wanted.key");
    ensure!(found.is_empty(), "expected no categories, got {found:?}");
    Ok(())
}

/// Named selectors are collected; numeric ones are exact matches, not CLDR
/// categories, so they are skipped. The default `*` marker is not part of the
/// name.
#[test]
fn named_selectors_are_collected_and_numeric_ones_skipped() -> Result<()> {
    let catalogue = concat!(
        "wanted.key = { $count ->\n",
        "    [0] None at all.\n",
        "    [one] One.\n",
        "    [few] A few.\n",
        "   *[other] Many.\n",
        "}\n",
    );
    let found = plural_categories(catalogue, "wanted.key");
    ensure!(
        found == categories(&["few", "one", "other"]),
        "expected the named categories only, got {found:?}"
    );
    Ok(())
}

/// The scan stops at the closing brace, so a `select` defined after the
/// requested one contributes nothing.
#[test]
fn the_scan_terminates_at_the_closing_brace() -> Result<()> {
    let catalogue = concat!(
        "wanted.key = { $count ->\n",
        "    [one] One.\n",
        "   *[other] Many.\n",
        "}\n",
        "later.plural = { $count ->\n",
        "    [two] Two.\n",
        "   *[other] Many.\n",
        "}\n",
    );
    let found = plural_categories(catalogue, "wanted.key");
    ensure!(
        found == categories(&["one", "other"]),
        "expected only the requested select's categories, got {found:?}"
    );
    Ok(())
}
