//! Tests for the locale catalogue registry and its fallback policy.
//!
//! These cover the registry's structural invariants and the deliberate
//! fallback rules that keep region and script variants distinct.

use std::collections::BTreeSet;
use std::str::FromStr;

use anyhow::{Result, ensure};
use ortho_config::LanguageIdentifier;
use rstest::rstest;

use netsuke::cli_localization::resolve_catalogue_tag;
use netsuke::localization::locales::{
    LocaleCatalogue, SOURCE_LOCALE, SUPPORTED_LOCALES, catalogue,
};

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
fn registry_tags_are_valid_language_identifiers() {
    for tag in registry_tags() {
        let parsed = LanguageIdentifier::from_str(tag)
            .unwrap_or_else(|error| panic!("{tag} should be a valid language identifier: {error}"));
        assert_eq!(
            parsed.to_string(),
            tag,
            "{tag} should already be in canonical form"
        );
    }
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
