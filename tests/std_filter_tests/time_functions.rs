//! Integration coverage for the stdlib `now()` clock seam.
//!
//! These cases render through the real `stdlib::register_with_config`
//! entrypoint, so they exercise the whole path a caller takes: configuration,
//! registration, and template evaluation. The unit cases in
//! `src/stdlib/time/tests.rs` assert the seam's internals; these assert that the
//! wiring between a caller's configuration and the registered helper is intact.
//!
//! Rendered strings are compared against the documented rendering rather than
//! against output captured from a run. `now()` renders through
//! `Iso8601::DEFAULT`, which includes separators, resolves the offset to the
//! minute, and writes UTC as `Z`; the zero fractional part is stripped by the
//! formatter.

use anyhow::{Context, Result, ensure};
use netsuke::stdlib::fixed_clock;
use rstest::rstest;
use test_support::tracing_capture::with_test_subscriber;
use time::{
    Duration, OffsetDateTime, UtcOffset, format_description::well_known::Iso8601, macros::datetime,
};
use tracing_subscriber::filter::LevelFilter;

use super::support::fallible;

/// Render `template` in an environment whose clock is fixed at `instant`.
fn render_with_fixed_clock(instant: OffsetDateTime, template: &str) -> Result<String> {
    let env = fallible::stdlib_env_with_clock(fixed_clock(instant))?;
    env.render_str(template, ())
        .with_context(|| format!("render `{template}`"))
}

/// Parse a timestamp rendered by `now()`.
fn parse_rendered(rendered: &str) -> Result<OffsetDateTime> {
    OffsetDateTime::parse(rendered, &Iso8601::DEFAULT)
        .with_context(|| format!("parse rendered timestamp `{rendered}`"))
}

#[rstest]
#[case::utc(datetime!(2026-06-08 12:00:00 UTC), "2026-06-08T12:00:00Z")]
#[case::epoch(datetime!(1970-01-01 00:00:00 UTC), "1970-01-01T00:00:00Z")]
#[case::non_utc(datetime!(2026-06-08 17:30:00 +05:30), "2026-06-08T12:00:00Z")]
fn now_uses_configured_clock(
    #[case] instant: OffsetDateTime,
    #[case] expected: &str,
) -> Result<()> {
    let rendered = render_with_fixed_clock(instant, "{{ now() }}")?;

    ensure!(
        rendered == expected,
        "expected `{expected}` but rendered `{rendered}`"
    );
    Ok(())
}

/// Without `with_clock`, `now()` still reads the host clock and reports UTC.
///
/// A tolerance against the real clock is the only oracle available for an
/// ambient read, and it is sufficient: the failure this guards against is a
/// default that is frozen or offset rather than live, which a seconds-scale
/// window detects immediately.
#[rstest]
fn now_without_a_clock_reads_the_host() -> Result<()> {
    let env = fallible::stdlib_env()?;
    let rendered = env
        .render_str("{{ now() }}", ())
        .context("render `now()`")?;
    let parsed = parse_rendered(&rendered)?;

    let delta = (OffsetDateTime::now_utc() - parsed).abs();
    ensure!(
        delta <= Duration::seconds(3),
        "ambient now() rendered `{rendered}`, which is {delta:?} from the host clock"
    );
    ensure!(
        parsed.offset() == UtcOffset::UTC,
        "ambient now() rendered `{rendered}` without a UTC offset"
    );
    Ok(())
}

/// An offset re-expresses the configured instant rather than shifting it.
///
/// Both assertions are load-bearing: the string comparison fixes the rendering,
/// and the instant comparison rejects an implementation that applies the offset
/// by moving the timestamp instead of by changing its representation.
#[rstest]
#[case::forward("+02:00", "2026-06-08T14:00:00+02:00")]
#[case::backward("-05:00", "2026-06-08T07:00:00-05:00")]
#[case::utc("Z", "2026-06-08T12:00:00Z")]
fn now_applies_offset_to_the_configured_clock(
    #[case] offset_spec: &str,
    #[case] expected: &str,
) -> Result<()> {
    let instant = datetime!(2026-06-08 12:00:00 UTC);
    let rendered =
        render_with_fixed_clock(instant, &format!("{{{{ now(offset='{offset_spec}') }}}}"))?;
    let parsed = parse_rendered(&rendered)?;

    ensure!(
        rendered == expected,
        "expected `{expected}` but rendered `{rendered}`"
    );
    ensure!(
        parsed.unix_timestamp() == instant.unix_timestamp(),
        "offset `{offset_spec}` moved the instant: `{rendered}`"
    );
    Ok(())
}

/// Registration reports which clock it installed, by provenance and nothing
/// else.
///
/// A wrongly wired clock is otherwise invisible: an injected clock that never
/// reached registration renders exactly as an ambient one would, so a test
/// asserting a pinned instant fails without saying why. The closed set is what
/// makes the label safe to record — it names the clock's source, never the
/// instant that clock would report.
#[rstest]
#[case::system(false, "system")]
#[case::injected(true, "injected")]
fn registration_reports_the_clock_source(
    #[case] inject: bool,
    #[case] expected: &str,
) -> Result<()> {
    let captured = with_test_subscriber(LevelFilter::DEBUG, |events| -> Result<Vec<String>> {
        let installed = if inject {
            fallible::stdlib_env_with_clock(fixed_clock(datetime!(2026-06-08 12:00:00 UTC)))
        } else {
            fallible::stdlib_env()
        };
        installed.context("registration should succeed")?;
        Ok(events.snapshot())
    })?;

    let registration_events: Vec<&String> = captured
        .iter()
        .filter(|event| event.contains("registered stdlib time helpers"))
        .collect();
    ensure!(
        registration_events.len() == 1,
        "expected one registration event but captured {registration_events:?}"
    );
    let event = registration_events
        .first()
        .copied()
        .context("the length check above leaves one event")?;
    ensure!(
        event.contains(&format!("clock_source=\"{expected}\"")),
        "the event should label the {expected} clock: {event}"
    );
    ensure!(
        !event.contains("2026-06-08"),
        "the event must not carry the instant a provider would report: {event}"
    );
    Ok(())
}
