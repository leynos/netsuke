//! Content checks over every catalogue in the locale registry.
//!
//! These read the embedded catalogue text rather than rendering messages, so
//! they cover properties the build-time audit does not: the CLDR plural
//! categories each locale declares, the bidi marking right-to-left catalogues
//! rely on, and the guarantee that a translation is not simply a copy of the
//! English source.

use std::collections::BTreeSet;

use netsuke::localization::locales::{SOURCE_LOCALE, SUPPORTED_LOCALES, catalogue};
use rstest::rstest;

/// Message used to demonstrate plural handling in every catalogue.
const PLURAL_EXAMPLE: &str = "example.files_processed";

/// Locales whose copy may legitimately match the English source, because they
/// are English.
const ENGLISH_LOCALES: [&str; 2] = ["en-GB", SOURCE_LOCALE];

fn catalogue_text(tag: &str) -> &'static str {
    catalogue(tag)
        .unwrap_or_else(|| panic!("{tag} should be in the locale registry"))
        .resource()
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

/// Every locale must declare exactly the CLDR plural categories its language
/// uses; a translator who drops `few` from Polish silently loses a form.
#[rstest]
#[case("ar", &["zero", "one", "two", "few", "many", "other"])]
#[case("cy", &["zero", "one", "two", "few", "many", "other"])]
#[case("gd", &["one", "two", "few", "other"])]
#[case("he", &["one", "two", "many", "other"])]
#[case("cs", &["one", "few", "many", "other"])]
#[case("pl", &["one", "few", "many", "other"])]
#[case("ru", &["one", "few", "many", "other"])]
#[case("uk", &["one", "few", "many", "other"])]
#[case("ro", &["one", "few", "other"])]
#[case("de", &["one", "other"])]
#[case("en-US", &["one", "other"])]
#[case("fa", &["one", "other"])]
#[case("hi", &["one", "other"])]
#[case("tr", &["one", "other"])]
#[case("hu", &["other"])]
#[case("id", &["other"])]
#[case("ja", &["other"])]
#[case("ko", &["other"])]
#[case("th", &["other"])]
#[case("vi", &["other"])]
#[case("zh-Hans", &["other"])]
#[case("zh-Hant", &["other"])]
fn plural_examples_declare_the_language_categories(#[case] tag: &str, #[case] expected: &[&str]) {
    let found = plural_categories(catalogue_text(tag), PLURAL_EXAMPLE);
    assert_eq!(
        found,
        categories(expected),
        "{tag} declares the wrong CLDR plural categories for {PLURAL_EXAMPLE}"
    );
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

/// A right-to-left message that opens with a Latin word, a bracket or a
/// placeable would otherwise take its paragraph direction from that token.
/// Prefixing the value with U+200F keeps the direction with the locale.
#[rstest]
#[case("ar")]
#[case("fa")]
#[case("he")]
fn rtl_catalogues_pin_paragraph_direction(#[case] tag: &str) {
    let text = catalogue_text(tag);
    for line in text.lines() {
        let Some((raw_id, raw_value)) = line.split_once('=') else {
            continue;
        };
        let (id, value) = (raw_id.trim(), raw_value.trim());
        if id.starts_with('#') || value.is_empty() || !value.chars().any(is_rtl) {
            continue;
        }
        let first = value.chars().next().unwrap_or(RTL_MARK);
        assert!(
            first == RTL_MARK || is_rtl(first),
            "{tag}: {id} contains right-to-left text but starts with {first:?}; \
             prefix the value with U+200F"
        );
    }
}

/// A catalogue that merely copies the English source is not a translation.
#[test]
fn translations_are_not_copies_of_the_source() {
    let source = catalogue_text(SOURCE_LOCALE);
    let sampled = ["cli.about", "manifest.parse", "status.state.pending"];
    for entry in SUPPORTED_LOCALES {
        if ENGLISH_LOCALES.contains(&entry.tag()) {
            continue;
        }
        for key in sampled {
            let translated = message_value(entry.resource(), key);
            let original = message_value(source, key);
            assert!(
                translated.is_some() && translated != original,
                "{}: {key} still matches the English source",
                entry.tag()
            );
        }
    }
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
