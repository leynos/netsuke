//! Locale-aware helpers for CLI messaging.
//!
//! Builds Fluent-backed localizers from the catalogue registry in
//! [`crate::locale_catalogues`], layering the requested locale over the
//! English source catalogue so any message a translation has not yet covered
//! still renders. Catalogue selection is by exact tag with the registry's
//! documented fallback rules, so region and script variants stay distinct.

use crate::locale_catalogues::{self as locales, LocaleCatalogue};
use ortho_config::LanguageIdentifier;
use ortho_config::{FluentLocalizer, FluentLocalizerBuilder, Localizer, NoOpLocalizer};
use std::str::FromStr;

struct LayeredLocalizer {
    primary: Box<dyn Localizer>,
    fallback: Box<dyn Localizer>,
}

impl LayeredLocalizer {
    fn new(primary: Box<dyn Localizer>, fallback: Box<dyn Localizer>) -> Self {
        Self { primary, fallback }
    }
}

impl Localizer for LayeredLocalizer {
    fn lookup(
        &self,
        id: &str,
        args: Option<&ortho_config::LocalizationArgs<'_>>,
    ) -> Option<String> {
        self.primary
            .lookup(id, args)
            .or_else(|| self.fallback.lookup(id, args))
    }
}

fn parse_locale_identifier(locale: &str) -> Option<LanguageIdentifier> {
    LanguageIdentifier::from_str(locale).ok()
}

fn build_en_localizer() -> Box<dyn Localizer> {
    match FluentLocalizer::with_en_us_defaults([locales::source_catalogue().resource()]) {
        Ok(localizer) => Box::new(localizer) as Box<dyn Localizer>,
        Err(err) => {
            tracing::warn!(error = %err, "failed to load default localization resources");
            Box::new(NoOpLocalizer::new()) as Box<dyn Localizer>
        }
    }
}

fn build_consumer_localizer(
    builder: FluentLocalizerBuilder,
    tag: &'static str,
    resource: &'static str,
) -> Option<Box<dyn Localizer>> {
    match builder
        .with_consumer_resources([resource])
        .disable_defaults()
        .try_build()
    {
        Ok(localizer) => Some(Box::new(localizer) as Box<dyn Localizer>),
        Err(err) => {
            // The build-time audit checks key parity but not Fluent syntax, so
            // a malformed catalogue first shows up here. Silence would look
            // like the locale simply having no translations.
            tracing::warn!(
                locale = tag,
                error = %err,
                "failed to load locale catalogue; falling back to the source locale"
            );
            None
        }
    }
}

/// Build a localizer for `catalogue`, layered over the English source copy.
///
/// `fallback` becomes the layered localizer's second tier. When the catalogue
/// itself fails to parse, it is handed straight back rather than rebuilt.
///
/// The bundle is built for the catalogue's own locale, not the requested one.
/// A request resolves to a catalogue that may name a different tag — `pt-AO`
/// serves European Portuguese — and Fluent takes plural rules and number
/// formatting from the bundle's locale. Building the bundle for `pt-AO` while
/// loading the `pt-PT` catalogue would pair one locale's messages with
/// another's rules.
fn build_layered_localizer(
    requested: &LanguageIdentifier,
    catalogue: &'static LocaleCatalogue,
    fallback: Box<dyn Localizer>,
) -> Box<dyn Localizer> {
    let locale = parse_locale_identifier(catalogue.tag()).unwrap_or_else(|| requested.clone());
    let builder = FluentLocalizer::builder(locale);
    match build_consumer_localizer(builder, catalogue.tag(), catalogue.resource()) {
        Some(primary) => Box::new(LayeredLocalizer::new(primary, fallback)),
        None => fallback,
    }
}

/// Build a CLI localizer with an English fallback.
///
/// `preferred_locale` is matched against the catalogue registry; unsupported or
/// unparseable tags fall back to the English source catalogue.
#[must_use]
pub fn build_localizer(preferred_locale: Option<&str>) -> Box<dyn Localizer> {
    let fallback = build_en_localizer();
    let Some(preferred) = preferred_locale else {
        return fallback;
    };
    let Some(locale) = parse_locale_identifier(preferred) else {
        // Callers normalize tags before reaching here, so an unparseable one
        // means the normalization was bypassed; say which tag was dropped.
        tracing::debug!(
            requested = preferred,
            effective = locales::SOURCE_LOCALE,
            reason = "unparseable",
            "locale request did not parse"
        );
        return fallback;
    };

    let catalogue = locales::resolve_catalogue(&locale);
    tracing::debug!(
        requested = preferred,
        effective = catalogue.tag(),
        "resolved locale catalogue"
    );
    if catalogue.tag() == locales::SOURCE_LOCALE {
        return fallback;
    }
    build_layered_localizer(&locale, catalogue, fallback)
}

#[cfg(test)]
#[path = "cli_localization_tracing_tests.rs"]
mod tracing_tests;
