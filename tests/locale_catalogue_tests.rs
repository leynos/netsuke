//! Content checks over every catalogue in the locale registry.
//!
//! These read the embedded catalogue text rather than rendering messages, so
//! they cover properties the build-time audit does not: the CLDR plural
//! categories each locale declares, the bidi marking right-to-left catalogues
//! rely on, and the guarantee that a translation is not simply a copy of the
//! English source.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use netsuke::localization::locales::{
    LocaleCatalogue, SOURCE_LOCALE, SUPPORTED_LOCALES, catalogue,
};
use rstest::rstest;

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

/// Collect the CLDR categories a `select` expression declares.
///
/// Variant lines look like `[one] …` or `*[other] …`; numeric selectors such
/// as `[0]` are exact matches rather than CLDR categories and are skipped.
fn plural_categories(text: &str, key: &str) -> BTreeSet<String> {
    let mut categories = BTreeSet::new();
    let mut inside = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some((id, _)) = trimmed.split_once('=')
            && id.trim() == key
        {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        if trimmed == "}" {
            break;
        }
        let variant = trimmed.trim_start_matches('*');
        if let Some(name) = variant
            .strip_prefix('[')
            .and_then(|rest| rest.split(']').next())
            && !name.chars().all(|ch| ch.is_ascii_digit())
        {
            categories.insert(name.to_owned());
        }
    }
    categories
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
            "{tag} declares the CLDR plural categories {found:?} for {PLURAL_EXAMPLE}, \
             expected {expected:?}"
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

const fn is_rtl(ch: char) -> bool {
    matches!(ch, '\u{0590}'..='\u{08FF}' | '\u{FB1D}'..='\u{FDFF}' | '\u{FE70}'..='\u{FEFF}')
}

/// Right-to-left marker that pins a message's paragraph direction.
const RTL_MARK: char = '\u{200F}';

/// Messages that are deliberately direction-neutral.
///
/// Each is either a bare technical token substituted into another message, or
/// an all-Latin diagnostic line. Pinning these to right-to-left would move a
/// Latin identifier to the wrong edge of the terminal, so they are exempt.
const DIRECTION_NEUTRAL: [&str; 5] = [
    // Clap's usage string, which names the binary and its Latin flags.
    "cli.usage",
    // Stream names, substituted into the command diagnostics.
    "stdlib.command.output.stream.stdout",
    "stdlib.command.output.stream.stderr",
    // An all-Latin diagnostic tag followed by its detail.
    "stdlib.which.args_error",
    // A `{symbol} {label}` composition template for accessible output.
    "semantic.prefix.rendered",
];

/// Whether `value` opens a `select` expression rather than carrying text.
///
/// The selector line renders nothing; its variants carry the text, and they
/// are checked separately.
fn opens_select(value: &str) -> bool {
    value.ends_with("->")
}

/// Every rendered fragment of a catalogue, as `(id, text)` pairs.
///
/// A message's own value is one fragment; each variant of a `select`
/// expression is another, because whichever variant Fluent picks becomes the
/// whole rendered string and so decides the paragraph direction on its own.
fn rendered_fragments(text: &str) -> Vec<(String, String)> {
    let mut fragments = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if let Some(variant) = trimmed.trim_start_matches('*').strip_prefix('[') {
            if let Some(rendered) = variant
                .split_once(']')
                .map(|(_, rest)| rest.trim())
                .filter(|rest| !rest.is_empty())
            {
                fragments.push((current.clone(), rendered.to_owned()));
            }
            continue;
        }
        let Some((id, raw_value)) = trimmed.split_once('=') else {
            continue;
        };
        id.trim().clone_into(&mut current);
        let value = raw_value.trim();
        if !value.is_empty() && !opens_select(value) {
            fragments.push((current.clone(), value.to_owned()));
        }
    }
    fragments
}

/// A right-to-left message that opens with a Latin word, a bracket or a
/// placeable would otherwise take its paragraph direction from that token.
/// Prefixing the value with U+200F keeps the direction with the locale.
///
/// Fluent wraps every interpolated value in bidi isolates, so a template built
/// only from placeables and punctuation — `[{ $state }] { $label }` — carries
/// no strong character at all and defaults to left-to-right. Those need the
/// mark just as much as a Latin-initial sentence does, so the check covers
/// every rendered fragment rather than only those with visible script.
#[rstest]
#[case("ar")]
#[case("fa")]
#[case("he")]
fn rtl_catalogues_pin_paragraph_direction(#[case] tag: &str) -> Result<()> {
    for (id, value) in rendered_fragments(catalogue_text(tag)?) {
        if DIRECTION_NEUTRAL.contains(&id.as_str()) {
            continue;
        }
        let first = value.chars().next().unwrap_or(RTL_MARK);
        ensure!(
            first == RTL_MARK || is_rtl(first),
            "{tag}: {id} renders text starting with {first:?}, which leaves the \
             paragraph direction to that character; prefix the value with U+200F"
        );
    }
    Ok(())
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
