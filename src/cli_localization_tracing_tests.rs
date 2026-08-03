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
    ensure!(
        event.contains("error"),
        "event must carry the parse error, got {event}"
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
