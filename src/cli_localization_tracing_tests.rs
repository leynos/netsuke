//! Tests for locale resolution and catalogue-load observability.
//!
//! These assert the events themselves rather than the rendered output, because
//! the fallback is deliberately invisible to a caller: an unresolvable locale
//! and a malformed catalogue both render English. The event is the only signal
//! that either happened, so losing it would make "why is this in English?"
//! unanswerable from a log.

use super::*;
use crate::test_tracing_capture::with_test_subscriber;
use anyhow::{Context, Result, ensure};
use rstest::rstest;
use tracing_subscriber::filter::LevelFilter;

/// Run `test` with a capturing subscriber and return the emitted events.
fn capture<T>(test: impl FnOnce() -> T) -> (T, Vec<String>) {
    with_test_subscriber(LevelFilter::TRACE, |captured| {
        let value = test();
        (value, captured.snapshot())
    })
}

/// Return the first captured event containing `needle`.
fn find_event<'a>(events: &'a [String], needle: &str) -> Result<&'a String> {
    events
        .iter()
        .find(|event| event.contains(needle))
        .with_context(|| format!("expected an event containing {needle:?}, got {events:?}"))
}

/// A tag that cannot parse as BCP 47 must say so, and name the tag it dropped.
///
/// Callers normalize tags before this point, so an unparseable one means the
/// normalization was bypassed; the event has to carry the offending value or
/// there is nothing to debug from.
#[rstest]
#[case("not a locale")]
#[case("!!")]
fn an_unparseable_locale_reports_why_it_was_dropped(#[case] requested: &str) -> Result<()> {
    let (_, events) = capture(|| build_localizer(Some(requested)));

    let event = find_event(&events, "locale request did not parse")?;
    ensure!(
        event.contains(requested),
        "event must name the requested tag {requested:?}, got {event}"
    );
    ensure!(
        event.contains("unparseable"),
        "event must carry reason=\"unparseable\", got {event}"
    );
    ensure!(
        event.contains(locales::SOURCE_LOCALE),
        "event must name the effective locale, got {event}"
    );
    Ok(())
}

/// A tag that parses but ships no catalogue of its own resolves through the
/// fallback rules, and the event records both ends of that decision.
#[test]
fn a_resolved_locale_reports_requested_and_effective_tags() -> Result<()> {
    let (_, events) = capture(|| build_localizer(Some("zh-TW")));

    let event = find_event(&events, "resolved locale catalogue")?;
    ensure!(
        event.contains("zh-TW"),
        "event must name the requested tag, got {event}"
    );
    ensure!(
        event.contains("zh-Hant"),
        "event must name the effective catalogue, got {event}"
    );
    Ok(())
}

/// A catalogue that fails to parse must be reported with its tag and error.
///
/// The build-time audit checks key parity but not Fluent syntax, so a malformed
/// catalogue first shows up here. Silence would be indistinguishable from the
/// locale simply having no translations.
#[test]
fn a_malformed_catalogue_reports_its_tag_and_error() -> Result<()> {
    // `= not a message` is junk to the Fluent parser: an entry with no
    // identifier. Fed through the same seam production uses.
    let malformed = "= not a message\n";
    let locale = parse_locale_identifier("fr").context("fr must parse as a language identifier")?;
    let builder = FluentLocalizer::builder(locale);

    let (built, events) = capture(|| build_consumer_localizer(builder, "fr", malformed));

    ensure!(
        built.is_none(),
        "a malformed catalogue must not yield a localizer"
    );
    let event = find_event(&events, "failed to load locale catalogue")?;
    ensure!(
        event.contains("fr"),
        "event must name the offending locale, got {event}"
    );
    // The rendered error itself, not merely a field named `error`: the field
    // name would match even if the value were empty, and the value is the part
    // a reader debugs from. `failed to parse … resources for` is the
    // `FluentLocalizerError::Parser` rendering.
    ensure!(
        event.contains("failed to parse") && event.contains("resources for fr"),
        "event must carry the rendered parse error, got {event}"
    );
    Ok(())
}

/// The well-formed path must stay quiet: a warning per shipped locale would
/// train readers to ignore the one that matters.
#[test]
fn a_well_formed_catalogue_reports_no_failure() -> Result<()> {
    let (_, events) = capture(|| build_localizer(Some("fr")));

    ensure!(
        !events
            .iter()
            .any(|event| event.contains("failed to load locale catalogue")),
        "a shipped catalogue must load without warning, got {events:?}"
    );
    Ok(())
}

/// A fallback-resolved request must use the catalogue's locale, not its own.
///
/// `pt-AO` ships no catalogue and resolves to `pt-PT`. Fluent reads plural
/// rules from the bundle's locale, so building the bundle for `pt-AO` while
/// loading `pt-PT` messages would pair one locale's text with another's rules.
/// Rendering identically to a direct `pt-PT` request is what shows they agree.
#[rstest]
#[case("pt-AO", "pt-PT")]
#[case("es-MX", "es-419")]
#[case("zh-TW", "zh-Hant")]
fn a_fallback_resolved_request_renders_as_its_catalogue(
    #[case] requested: &str,
    #[case] catalogue_tag: &str,
) -> Result<()> {
    let via_fallback = build_localizer(Some(requested));
    let via_catalogue = build_localizer(Some(catalogue_tag));
    let via_source = build_localizer(Some(locales::SOURCE_LOCALE));

    for count in [0_i64, 1, 2, 5] {
        let mut args = ortho_config::LocalizationArgs::new();
        args.insert("count", fluent_bundle::FluentValue::from(count));
        let fallback_text = via_fallback.lookup(
            crate::localization::keys::EXAMPLE_FILES_PROCESSED,
            Some(&args),
        );
        let catalogue_text = via_catalogue.lookup(
            crate::localization::keys::EXAMPLE_FILES_PROCESSED,
            Some(&args),
        );
        ensure!(
            fallback_text == catalogue_text,
            "{requested} must render as {catalogue_tag} for count {count}: {fallback_text:?} vs {catalogue_text:?}"
        );
        // Agreement alone would also hold if both fell through to English, so
        // this is what shows the requested catalogue was actually loaded.
        let source_text = via_source.lookup(
            crate::localization::keys::EXAMPLE_FILES_PROCESSED,
            Some(&args),
        );
        ensure!(
            fallback_text != source_text,
            "{requested} rendered the English source rather than {catalogue_tag} for count {count}: {fallback_text:?}"
        );
        ensure!(
            fallback_text.is_some_and(|text| !text.trim().is_empty()),
            "{requested} rendered nothing for count {count}"
        );
    }
    Ok(())
}

/// An unsupported locale must be visible at the level a normal run uses.
///
/// The startup filter is `WARN` when JSON mode is off, so a fallback reported
/// only at `DEBUG` would be invisible without `--verbose` — a run would render
/// English with nothing said about it. These assert the level, not just the
/// event, because that is the part that decides visibility.
#[rstest]
// A tag that parses but ships no catalogue, and whose language ships none.
#[case("is-IS")]
// A tag that cannot parse at all.
#[case("not a locale")]
fn an_english_fallback_is_reported_at_warn(#[case] requested: &str) -> Result<()> {
    let (_, at_warn) = with_test_subscriber(LevelFilter::WARN, |captured| {
        let localizer = build_localizer(Some(requested));
        (localizer, captured.snapshot())
    });

    ensure!(
        at_warn
            .iter()
            .any(|event| event.contains("falling back to the source locale")),
        "a fallback to English must be reported at WARN, got {at_warn:?}"
    );
    ensure!(
        at_warn.iter().any(|event| event.contains(requested)),
        "the event must name the requested tag {requested:?}, got {at_warn:?}"
    );
    Ok(())
}

/// A supported locale must stay quiet at `WARN`.
///
/// Otherwise the warning above would fire on every ordinary run and stop
/// meaning anything.
#[test]
fn a_supported_locale_warns_about_nothing() -> Result<()> {
    let (_, at_warn) = with_test_subscriber(LevelFilter::WARN, |captured| {
        let localizer = build_localizer(Some("fr"));
        (localizer, captured.snapshot())
    });

    ensure!(
        at_warn.is_empty(),
        "a shipped catalogue must not warn, got {at_warn:?}"
    );
    Ok(())
}

/// A request that resolves within its own language is supported, not a
/// fallback.
///
/// `en-us` normalizes to `en-US` and resolves to the source catalogue, so
/// warning about it would report a supported request as unsupported — and a
/// warning that fires on correct input trains readers to ignore it. Bare `en`
/// is the same case reached by a different route: the language fallback policy
/// sends it to `en-US` deliberately, and an English speaker who asked for
/// English got English.
#[rstest]
#[case("en-US")]
#[case("en-us")]
#[case("EN-US")]
#[case("en")]
#[case("en-AU")]
fn a_source_locale_spelling_warns_about_nothing(#[case] requested: &str) -> Result<()> {
    let (_, at_warn) = with_test_subscriber(LevelFilter::WARN, |captured| {
        let localizer = build_localizer(Some(requested));
        (localizer, captured.snapshot())
    });

    ensure!(
        at_warn.is_empty(),
        "{requested} resolves to the source catalogue and must not warn, got {at_warn:?}"
    );
    Ok(())
}
