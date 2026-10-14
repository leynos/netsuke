//! Unit tests for the adapter's `Location` header parse and its diagnostics.
//!
//! Reading a response header is transport work, so the adapter — not the chain
//! — resolves the `Location` value and owns the "absent" and "present but
//! unparsable" failures. These cases pin the resolver, the closed telemetry
//! reason each failure is counted under, the localized message it renders, and
//! the four bounded trace fields the refusal logs. They live here rather than
//! in the parent so the adapter's tests stay within the repository's 400-line
//! cap; the parent reaches them by declaring this module.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, bail, ensure};
use metrics_util::debugging::DebuggingRecorder;
use rstest::rstest;
use test_support::{
    fluent::normalize_fluent_isolates,
    localizer::{EnLocalizer, en_localizer},
    tracing_capture::with_test_subscriber,
};
use tracing_subscriber::filter::LevelFilter;

use super::*;

/// A resolvable header becomes the absolute target the chain judges.
///
/// A relative value resolves against the current URL and an absolute one keeps
/// its own origin, so the chain never has to parse or join anything. Resolution
/// is not redaction: a same-origin relative value keeps the current URL's
/// userinfo, which the chain strips only when the origin actually changes.
#[rstest]
#[case("http://blocked.example/next", "http://blocked.example/next")]
#[case(
    "/relative",
    "http://redirect-user:redirect-secret@allowed.example/relative"
)]
fn resolvable_headers_become_absolute_targets(
    #[case] header: &str,
    #[case] expected: &str,
) -> Result<()> {
    let current = credentialed_current_url()?;
    let policy = NetworkPolicy::default();
    let chain = RedirectChain::new(&current, &policy);

    let target = resolve_location(&chain, Some(header))
        .map_err(|failure| anyhow::anyhow!("{header:?} should resolve, got {failure:?}"))?;
    ensure!(
        target.as_str() == expected,
        "{header:?} should resolve to {expected}, got {target}",
    );
    Ok(())
}

/// An absent header and an unresolvable one are distinct adapter failures.
///
/// Neither reaches the chain: the header parse is transport work, so these are
/// the only two failures the adapter diagnoses without asking the chain.
#[rstest]
#[case(None, LocationFailure::Missing)]
#[case(Some("http://[::1"), LocationFailure::Unparsable)]
fn unusable_headers_report_their_own_failure(
    #[case] header: Option<&str>,
    #[case] expected: LocationFailure,
) -> Result<()> {
    let current = credentialed_current_url()?;
    let policy = NetworkPolicy::default();
    let chain = RedirectChain::new(&current, &policy);

    let Err(failure) = resolve_location(&chain, header) else {
        bail!("an unusable header must not resolve: {header:?}");
    };
    ensure!(
        failure == expected,
        "{header:?} should report {expected:?}, got {failure:?}",
    );
    Ok(())
}

/// Each header failure is counted under its own closed telemetry category.
#[rstest]
fn every_location_failure_has_a_distinct_closed_category() {
    let reasons =
        [LocationFailure::Missing, LocationFailure::Unparsable].map(LocationFailure::reason);
    let unique = reasons.iter().copied().collect::<BTreeSet<_>>();

    assert_eq!(
        unique.len(),
        reasons.len(),
        "each header failure needs its own telemetry category: {reasons:?}",
    );
    for reason in reasons {
        assert!(
            telemetry::FETCH_REDIRECT_FAILURE_VALUES.contains(&reason),
            "category {reason} must come from the declared vocabulary",
        );
    }
}

/// A header failure is counted and logged exactly like a chain refusal.
///
/// The two paths are distinguishable only by the closed `redirect_failure`
/// reason, so a refusal and a bad header carry the same four bounded fields.
#[rstest]
fn location_failures_are_counted_and_logged_with_four_fields() -> Result<()> {
    let current = credentialed_current_url()?;
    let policy = NetworkPolicy::default();
    let chain = RedirectChain::new(&current, &policy);
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    let samples = metrics::with_local_recorder(&recorder, || {
        let (events, _rendered) = with_test_subscriber(LevelFilter::WARN, |captured| {
            let rendered = report_location_failure(&chain, LocationFailure::Missing, 1).to_string();
            (captured.snapshot(), rendered)
        });
        events
    });

    let totals = counter_totals(
        &collect_samples(snapshotter.snapshot().into_vec()),
        telemetry::FETCH_REDIRECT_TOTAL,
    );
    ensure!(
        totals
            == BTreeMap::from([(
                vec![
                    ("outcome".to_owned(), "rejected".to_owned()),
                    ("redirect_failure".to_owned(), "location_missing".to_owned()),
                ],
                1,
            )]),
        "a header failure must be counted under its closed reason: {totals:?}",
    );

    let [event] = samples.as_slice() else {
        bail!("a header failure must emit exactly one warning: {samples:?}");
    };
    for field in [
        "operation=\"fetch\"",
        "redirect_outcome=\"rejected\"",
        "redirect_failure=\"location_missing\"",
        "hop=1",
    ] {
        ensure!(
            event.contains(field),
            "the refusal event must carry {field}: {event}",
        );
    }
    // Pinning the field *names* is what enforces ADR-023's four-field bound: a
    // check for the URL's value alone would still pass if the event named the
    // response in a field of its own.
    for field in ["location=", "url=", "host=", "userinfo="] {
        ensure!(
            !event.contains(field),
            "the refusal event must not carry {field}: {event}",
        );
    }
    ensure!(
        !event.contains("allowed.example"),
        "the refusal event must not name the URL: {event}",
    );
    for secret in SECRETS {
        ensure!(
            !event.contains(secret),
            "the refusal event must not disclose {secret}: {event}",
        );
    }
    Ok(())
}

/// Both header failures render the localized message without credentials.
#[rstest]
fn location_failures_render_a_redacted_diagnostic(en_localizer: EnLocalizer) -> Result<()> {
    let _localizer = en_localizer;
    let current = credentialed_current_url()?;
    let policy = NetworkPolicy::default();
    let chain = RedirectChain::new(&current, &policy);
    let expected = [
        (
            LocationFailure::Missing,
            "did not include a Location header",
        ),
        (LocationFailure::Unparsable, "Invalid redirect location"),
    ];

    for (failure, fragment) in expected {
        let rendered =
            normalize_fluent_isolates(&report_location_failure(&chain, failure, 1).to_string());
        ensure!(
            rendered.contains(fragment),
            "the {failure:?} diagnostic should mention '{fragment}', got {rendered}",
        );
        for secret in SECRETS {
            ensure!(
                !rendered.contains(secret),
                "the {failure:?} diagnostic must not disclose {secret}: {rendered}",
            );
        }
    }
    Ok(())
}
