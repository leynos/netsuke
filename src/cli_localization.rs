//! Locale-aware helpers for CLI messaging.
//!
//! Provides Fluent-backed localizers with an English fallback and
//! consumer-provided Spanish translations to validate localisation support.

use ortho_config::LanguageIdentifier;
use ortho_config::{FluentLocalizer, FluentLocalizerBuilder, Localizer, NoOpLocalizer};
use std::str::FromStr;

const NETSUKE_EN_US: &str = include_str!("../locales/en-US/messages.ftl");
const NETSUKE_ES_ES: &str = include_str!("../locales/es-ES/messages.ftl");

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

fn parse_locale(locale: &str) -> Option<LanguageIdentifier> {
    LanguageIdentifier::from_str(locale).ok()
}

fn build_en_localizer() -> Box<dyn Localizer> {
    FluentLocalizer::with_en_us_defaults([NETSUKE_EN_US]).map_or_else(
        |_| Box::new(NoOpLocalizer::new()) as Box<dyn Localizer>,
        |localizer| Box::new(localizer) as Box<dyn Localizer>,
    )
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

fn locale_language(locale: &LanguageIdentifier) -> &str {
    locale.language.as_str()
}

/// Build a CLI localizer with an English fallback.
#[must_use]
pub fn build_localizer(preferred_locale: Option<&str>) -> Box<dyn Localizer> {
    let fallback = build_en_localizer();
    let Some(preferred) = preferred_locale else {
        return fallback;
    };
    let Some(locale) = parse_locale(preferred) else {
        return fallback;
    };

    if locale_language(&locale) == "es" {
        let builder = FluentLocalizer::builder(locale);
        if let Some(primary) = build_consumer_localizer(builder, NETSUKE_ES_ES) {
            return Box::new(LayeredLocalizer::new(primary, fallback));
        }
    }

    fallback
}
