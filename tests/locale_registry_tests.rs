//! Tests for the locale catalogue registry and its fallback policy.
//!
//! These cover the registry's structural invariants and the deliberate
//! fallback rules that keep region and script variants distinct.

use std::collections::BTreeSet;
use std::str::FromStr;

use anyhow::{Context, Result, ensure};
use ortho_config::LanguageIdentifier;
use proptest::prelude::*;
use rstest::rstest;

use netsuke::localization::locales::{
    LocaleCatalogue, SOURCE_LOCALE, SUPPORTED_LOCALES, catalogue, resolve_catalogue,
    source_catalogue,
};

/// The tag of the catalogue serving `requested`.
///
/// Mirrors what `cli_localization::build_localizer` does with a requested
/// locale: parse it, resolve through the registry, and fall back to the source
/// catalogue when the tag will not parse.
fn resolve_catalogue_tag(requested: &str) -> &'static str {
    LanguageIdentifier::from_str(requested)
        .as_ref()
        .map_or_else(|_| source_catalogue(), resolve_catalogue)
        .tag()
}

fn registry_tags() -> Vec<&'static str> {
    SUPPORTED_LOCALES.iter().map(LocaleCatalogue::tag).collect()
}

#[test]
fn registry_contains_the_source_locale() {
    assert!(
        catalogue(SOURCE_LOCALE).is_some(),
        "the registry must contain the source locale {SOURCE_LOCALE}"
    );
}

#[test]
fn registry_tags_are_unique_and_sorted() {
    let tags = registry_tags();
    let unique: BTreeSet<&str> = tags.iter().copied().collect();
    assert_eq!(unique.len(), tags.len(), "registry tags must be unique");
    let mut sorted = tags.clone();
    sorted.sort_unstable();
    assert_eq!(tags, sorted, "registry tags must be declared in tag order");
}

#[test]
fn registry_tags_are_valid_language_identifiers() -> Result<()> {
    for tag in registry_tags() {
        let parsed = LanguageIdentifier::from_str(tag)
            .with_context(|| format!("{tag} should be a valid language identifier"))?;
        let canonical = parsed.to_string();
        ensure!(
            canonical == tag,
            "{tag} should already be in canonical form, canonicalized to {canonical}"
        );
    }
    Ok(())
}

#[test]
fn every_registry_catalogue_embeds_content() {
    for entry in SUPPORTED_LOCALES {
        assert!(
            !entry.resource().trim().is_empty(),
            "catalogue {} should not be empty",
            entry.tag()
        );
    }
}

/// Every shipped tag resolves to its own catalogue rather than a relative.
#[test]
fn every_registry_tag_resolves_to_itself() -> Result<()> {
    for tag in registry_tags() {
        let resolved = resolve_catalogue_tag(tag);
        ensure!(
            resolved == tag,
            "expected {tag} to resolve to itself, got {resolved}"
        );
    }
    Ok(())
}

#[rstest]
// Unknown languages fall back to the source locale.
#[case("tlh", SOURCE_LOCALE)]
#[case("xx-YY", SOURCE_LOCALE)]
// Unparseable input falls back to the source locale.
#[case("not a locale", SOURCE_LOCALE)]
#[case("", SOURCE_LOCALE)]
fn unsupported_locales_fall_back_to_the_source(#[case] requested: &str, #[case] expected: &str) {
    assert_eq!(resolve_catalogue_tag(requested), expected);
}

/// The registry's documented fallback rules, exercised tag by tag.
///
/// The point of these cases is that region and script variants which differ in
/// substance stay apart: a Mexican request must not land on the Spain
/// catalogue, and a Taiwanese one must not land on the Simplified catalogue.
#[rstest]
// Spanish: Spain keeps its own copy, every other region shares es-419.
#[case("es", "es-ES")]
#[case("es-ES", "es-ES")]
#[case("es-419", "es-419")]
#[case("es-MX", "es-419")]
#[case("es-AR", "es-419")]
// Portuguese: Brazil and Portugal stay apart; other regions take European.
#[case("pt", "pt-PT")]
#[case("pt-BR", "pt-BR")]
#[case("pt-PT", "pt-PT")]
#[case("pt-AO", "pt-PT")]
// Chinese: script wins, and regions map to the script they conventionally use.
#[case("zh", "zh-Hans")]
#[case("zh-Hans", "zh-Hans")]
#[case("zh-Hant", "zh-Hant")]
#[case("zh-CN", "zh-Hans")]
#[case("zh-SG", "zh-Hans")]
#[case("zh-TW", "zh-Hant")]
#[case("zh-HK", "zh-Hant")]
#[case("zh-Hant-TW", "zh-Hant")]
#[case("zh-Hans-CN", "zh-Hans")]
// English: the bare tag keeps the source; other regions prefer British copy.
#[case("en", "en-US")]
#[case("en-US", "en-US")]
#[case("en-GB", "en-GB")]
#[case("en-AU", "en-GB")]
#[case("en-IE", "en-GB")]
// The Norwegian macrolanguage resolves to Bokmål.
#[case("no", "nb")]
#[case("nb-NO", "nb")]
// Languages shipping one catalogue serve all their regions from it.
#[case("fr-CA", "fr")]
#[case("de-AT", "de")]
#[case("ja-JP", "ja")]
#[case("pl-PL", "pl")]
fn fallback_rules_keep_regional_variants_distinct(#[case] requested: &str, #[case] expected: &str) {
    assert_eq!(
        resolve_catalogue_tag(requested),
        expected,
        "{requested} should resolve to {expected}"
    );
}

/// Resolution must be total: whatever tag arrives, the caller gets a
/// catalogue that is actually in the registry, never a dangling or synthesised
/// one. Fixed cases cannot cover the tag space, so this is a property.
#[test]
fn resolution_always_lands_on_a_registry_catalogue() {
    let tags: BTreeSet<&str> = registry_tags().into_iter().collect();
    proptest!(|(requested in "\\PC{0,24}")| {
        let resolved = resolve_catalogue_tag(&requested);
        prop_assert!(
            tags.contains(resolved),
            "{requested:?} resolved to {resolved:?}, which is not in the registry"
        );
    });
}

/// A well-formed tag whose language ships a catalogue must reach that
/// language, never a different one. Generated from the registry so a new
/// locale is covered without editing the test.
#[test]
fn a_known_language_never_resolves_to_another_language() {
    let languages: Vec<String> = registry_tags()
        .iter()
        .map(|tag| tag.split('-').next().unwrap_or(tag).to_owned())
        .collect();
    let count = languages.len();
    proptest!(|(index in 0usize..1024, region in "[A-Z]{2}")| {
        let Some(language) = languages.get(index.wrapping_rem(count)) else {
            return Ok(());
        };
        let requested = format!("{language}-{region}");
        let Ok(parsed) = LanguageIdentifier::from_str(&requested) else {
            return Ok(());
        };
        let resolved = resolve_catalogue(&parsed).tag();
        let resolved_language = resolved.split('-').next().unwrap_or(resolved);
        prop_assert_eq!(
            resolved_language,
            language.as_str(),
            "{} resolved to {}, changing language",
            requested,
            resolved
        );
    });
}
