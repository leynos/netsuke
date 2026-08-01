//! Authoritative registry of the locale catalogues shipped with Netsuke.
//!
//! This module is the single source of truth for which locales exist. The
//! embedded Fluent resources, the build-time key audit, the `Cargo.toml`
//! `ortho_config` metadata check, and the test suite all read the registry
//! declared here rather than repeating locale lists of their own.
//!
//! Netsuke ships one catalogue per locale tag. Requests for a tag without its
//! own catalogue are resolved through deliberate fallback rules so that
//! regional and script variants which genuinely differ — `es-419` versus
//! `es-ES`, `pt-BR` versus `pt-PT`, `zh-Hans` versus `zh-Hant` — are never
//! collapsed into a single generic language catalogue.

use ortho_config::LanguageIdentifier;

/// Locale tag of the source catalogue, used as the ultimate fallback.
pub const SOURCE_LOCALE: &str = "en-US";

/// A Fluent catalogue embedded in the binary.
#[derive(Debug, Clone, Copy)]
pub struct LocaleCatalogue {
    tag: &'static str,
    resource: &'static str,
}

impl LocaleCatalogue {
    /// BCP 47 tag naming this catalogue, for example `pt-BR`.
    #[must_use]
    pub const fn tag(&self) -> &'static str {
        self.tag
    }

    /// Fluent source text embedded from `locales/<tag>/messages.ftl`.
    #[must_use]
    pub const fn resource(&self) -> &'static str {
        self.resource
    }
}

/// Declare the supported locales and embed their catalogues.
///
/// Each tag must have a matching `locales/<tag>/messages.ftl` file; the
/// `include_str!` expansion fails the build when one is missing, which keeps
/// the registry and the on-disk catalogues in step.
macro_rules! define_locales {
    ($($tag:literal),+ $(,)?) => {
        /// Every locale catalogue shipped with Netsuke, ordered by tag.
        pub const SUPPORTED_LOCALES: &[LocaleCatalogue] = &[
            $(LocaleCatalogue {
                tag: $tag,
                resource: include_str!(concat!("../../locales/", $tag, "/messages.ftl")),
            }),+
        ];
    };
}

define_locales![
    "cs", "da", "de", "el", "en-GB", "en-US", "es-419", "es-ES", "fi", "fr", "hu", "it", "nb",
    "nl", "pl", "pt-BR", "pt-PT", "ro", "ru", "sv", "uk",
];

/// Fallback policy for a language that ships more than one catalogue, or whose
/// requests should be redirected to a differently named catalogue.
struct LanguageFallback {
    /// Language subtag the policy applies to, for example `zh`.
    language: &'static str,
    /// Script or region subtags mapped to a specific catalogue.
    subtags: &'static [(&'static str, &'static str)],
    /// Catalogue used for any other region of this language.
    other_region: &'static str,
    /// Catalogue used when the request names no region or script.
    bare: &'static str,
}

/// Deliberate fallback rules for ambiguous or aliased languages.
///
/// Languages absent from this table resolve through the unique-language rule in
/// [`resolve_catalogue`], which is sufficient while they ship exactly one
/// catalogue.
const LANGUAGE_FALLBACKS: &[LanguageFallback] = &[
    LanguageFallback {
        language: "en",
        // British copy is the better fit for English outside the United
        // States; the bare tag keeps the source locale.
        subtags: &[("US", "en-US"), ("GB", "en-GB")],
        other_region: "en-GB",
        bare: "en-US",
    },
    LanguageFallback {
        language: "es",
        subtags: &[("ES", "es-ES"), ("419", "es-419")],
        // Spanish-speaking regions outside Spain share the Latin American
        // catalogue.
        other_region: "es-419",
        bare: "es-ES",
    },
    LanguageFallback {
        language: "pt",
        subtags: &[("BR", "pt-BR"), ("PT", "pt-PT")],
        other_region: "pt-PT",
        bare: "pt-PT",
    },
    LanguageFallback {
        language: "zh",
        subtags: &[
            ("Hans", "zh-Hans"),
            ("Hant", "zh-Hant"),
            ("CN", "zh-Hans"),
            ("SG", "zh-Hans"),
            ("MY", "zh-Hans"),
            ("TW", "zh-Hant"),
            ("HK", "zh-Hant"),
            ("MO", "zh-Hant"),
        ],
        other_region: "zh-Hans",
        bare: "zh-Hans",
    },
    LanguageFallback {
        // Macrolanguage Norwegian resolves to the Bokmål catalogue.
        language: "no",
        subtags: &[],
        other_region: "nb",
        bare: "nb",
    },
];

/// Look up a catalogue by exact tag.
#[must_use]
pub fn catalogue(tag: &str) -> Option<&'static LocaleCatalogue> {
    SUPPORTED_LOCALES.iter().find(|entry| entry.tag == tag)
}

/// Catalogue used when nothing better matches.
///
/// Placeholder used only if the registry were ever built without the source
/// locale; a test asserts that this cannot happen.
const EMPTY_SOURCE: LocaleCatalogue = LocaleCatalogue {
    tag: SOURCE_LOCALE,
    resource: "",
};

/// The source catalogue, which every other locale falls back to.
///
/// [`SOURCE_LOCALE`] is a registry member, so the fallback arms below are
/// unreachable in practice; they exist to keep this a panic-free path.
#[must_use]
pub fn source_catalogue() -> &'static LocaleCatalogue {
    catalogue(SOURCE_LOCALE).unwrap_or(&EMPTY_SOURCE)
}

fn fallback_for(language: &str) -> Option<&'static LanguageFallback> {
    LANGUAGE_FALLBACKS
        .iter()
        .find(|entry| entry.language == language)
}

/// Catalogue for a language that ships exactly one variant, for example `fr`
/// serving a request for `fr-CA`.
fn unique_language_catalogue(language: &str) -> Option<&'static LocaleCatalogue> {
    let mut matches = SUPPORTED_LOCALES
        .iter()
        .filter(|entry| tag_language(entry.tag) == language);
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn tag_language(tag: &str) -> &str {
    tag.split('-').next().unwrap_or(tag)
}

fn subtag_catalogue(
    fallback: &LanguageFallback,
    subtag: Option<&str>,
) -> Option<&'static LocaleCatalogue> {
    let requested = subtag?;
    fallback
        .subtags
        .iter()
        .find(|(key, _)| *key == requested)
        .and_then(|(_, tag)| catalogue(tag))
}

/// Resolve the catalogue serving `locale`, applying the documented fallback
/// rules and finishing at the source locale.
///
/// Resolution order is exact tag, then the language's script or region rule,
/// then the language's default, then the sole catalogue for that language, and
/// finally [`SOURCE_LOCALE`].
///
/// # Examples
///
/// ```rust
/// use netsuke::localization::locales::resolve_catalogue;
/// use std::str::FromStr;
///
/// let parse = |tag: &str| {
///     ortho_config::LanguageIdentifier::from_str(tag).expect("valid language identifier")
/// };
/// // A Latin American region resolves to the shared es-419 catalogue.
/// assert_eq!(resolve_catalogue(&parse("es-MX")).tag(), "es-419");
/// // Spain keeps its own.
/// assert_eq!(resolve_catalogue(&parse("es-ES")).tag(), "es-ES");
/// ```
#[must_use]
pub fn resolve_catalogue(locale: &LanguageIdentifier) -> &'static LocaleCatalogue {
    let language = locale.language.as_str();
    let script = locale.script.map(|script| script.to_string());
    let region = locale.region.map(|region| region.to_string());

    if let Some(exact) = catalogue(&locale.to_string()) {
        return exact;
    }
    if let Some(fallback) = fallback_for(language) {
        return resolve_via_fallback(fallback, script.as_deref(), region.as_deref());
    }
    unique_language_catalogue(language).unwrap_or_else(source_catalogue)
}

fn resolve_via_fallback(
    fallback: &LanguageFallback,
    script: Option<&str>,
    region: Option<&str>,
) -> &'static LocaleCatalogue {
    subtag_catalogue(fallback, script)
        .or_else(|| subtag_catalogue(fallback, region))
        .or_else(|| {
            let tag = if region.is_some() {
                fallback.other_region
            } else {
                fallback.bare
            };
            catalogue(tag)
        })
        .unwrap_or_else(source_catalogue)
}
