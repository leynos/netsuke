//! Locale-aware helpers for CLI messaging.
//!
//! Builds Fluent-backed localizers from the catalogue registry in
//! [`crate::localization::locales`], layering the requested locale over the
//! English source catalogue so any message a translation has not yet covered
//! still renders. Catalogue selection is by exact tag with the registry's
//! documented fallback rules, so region and script variants stay distinct.

use crate::localization::locales::{self, LocaleCatalogue};
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
    resource: &'static str,
) -> Option<Box<dyn Localizer>> {
    builder
        .with_consumer_resources([resource])
        .disable_defaults()
        .try_build()
        .ok()
        .map(|localizer| Box::new(localizer) as Box<dyn Localizer>)
}

/// Resolve the catalogue Netsuke will use for `preferred_locale`.
///
/// Unparseable tags resolve to the source catalogue, matching the behaviour of
/// [`build_localizer`].
///
/// # Examples
///
/// ```rust
/// use netsuke::cli_localization::resolve_catalogue_tag;
///
/// assert_eq!(resolve_catalogue_tag("es-ES"), "es-ES");
/// assert_eq!(resolve_catalogue_tag("not a locale"), "en-US");
/// ```
#[must_use]
pub fn resolve_catalogue_tag(preferred_locale: &str) -> &'static str {
    parse_locale_identifier(preferred_locale)
        .map_or_else(locales::source_catalogue, |locale| {
            locales::resolve_catalogue(&locale)
        })
        .tag()
}

/// Build a localizer for `catalogue`, layered over the English source copy.
///
/// `fallback` is consumed as the layered localizer's second tier. When the
/// catalogue itself fails to parse there is no fallback left to hand back, so a
/// fresh English localizer is built for that rare path.
fn build_layered_localizer(
    locale: LanguageIdentifier,
    catalogue: &'static LocaleCatalogue,
    fallback: Box<dyn Localizer>,
) -> Box<dyn Localizer> {
    let builder = FluentLocalizer::builder(locale);
    build_consumer_localizer(builder, catalogue.resource())
        .map_or_else(build_en_localizer, |primary| {
            Box::new(LayeredLocalizer::new(primary, fallback)) as Box<dyn Localizer>
        })
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
        return fallback;
    };

    let catalogue = locales::resolve_catalogue(&locale);
    if catalogue.tag() == locales::SOURCE_LOCALE {
        return fallback;
    }
    build_layered_localizer(locale, catalogue, fallback)
}
