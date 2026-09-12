//! Unit tests for the redirect adapter's diagnostics and chain budget.
//!
//! The adapter owns everything the pure chain cannot: the localized message a
//! rejection becomes, the closed telemetry category it is counted under, and
//! the single deadline every hop draws from. These cases pin all three without
//! a socket, so a change to the wording, the vocabulary, or the budget is
//! caught here rather than in a fixture that only sees one chain.

use std::collections::BTreeSet;

use anyhow::{Context, Result, bail, ensure};
use insta::assert_snapshot;
use rstest::rstest;
use test_support::{
    fluent::normalize_fluent_isolates,
    localizer::{EnLocalizer, en_localizer},
    tracing_capture::with_test_subscriber,
};
use tracing_subscriber::filter::LevelFilter;

use super::super::redirect_chain::FETCH_REDIRECT_LIMIT;
use super::*;
use crate::snapshot_test_support::snapshot_settings;

/// Credentialed URL used as the current URL of a refused redirect.
const CREDENTIALED_CURRENT: &str = "http://redirect-user:redirect-secret@allowed.example/start";
/// Credentialed URL used as the refused target of a redirect.
const CREDENTIALED_TARGET: &str = "http://redirect-user:redirect-secret@blocked.example/next";
/// Userinfo fragments no diagnostic may disclose.
const SECRETS: [&str; 2] = ["redirect-user", "redirect-secret"];

/// One rejection paired with the localized fragment its diagnostic must carry.
type RejectionCase = (RedirectRejection, String);

/// Parse one test URL.
///
/// # Errors
///
/// Returns an error when `raw` is not a well-formed URL.
fn parse_url(raw: &str) -> Result<Url> {
    Url::parse(raw).with_context(|| format!("test URL should parse: {raw}"))
}

/// Build every rejection variant from credentialed URLs.
///
/// Each case is paired with a fragment that only its own localized diagnostic
/// carries, so a rejection reported through the wrong message fails the test.
///
/// # Errors
///
/// Returns an error when a test URL is malformed or when the default policy
/// unexpectedly permits the target that has to carry a violation.
fn every_rejection() -> Result<Vec<RejectionCase>> {
    let current = parse_url(CREDENTIALED_CURRENT)?;
    let target = parse_url(CREDENTIALED_TARGET)?;
    let Err(violation) = NetworkPolicy::default().evaluate(&target) else {
        bail!("the default policy should refuse {CREDENTIALED_TARGET}");
    };
    let limit = format!("Redirect limit of {FETCH_REDIRECT_LIMIT} exceeded");

    Ok(vec![
        (
            RedirectRejection::LocationMissing {
                current_url: current.clone(),
            },
            String::from("did not include a Location header"),
        ),
        (
            RedirectRejection::LocationInvalid {
                current_url: current.clone(),
            },
            String::from("Invalid redirect location"),
        ),
        (
            RedirectRejection::CredentialsNotRemovable {
                current_url: current.clone(),
            },
            String::from("Credentials could not be removed"),
        ),
        (
            RedirectRejection::LimitExceeded {
                target: target.clone(),
                limit: FETCH_REDIRECT_LIMIT,
            },
            limit,
        ),
        (
            RedirectRejection::Loop {
                target: target.clone(),
            },
            String::from("Redirect loop detected at"),
        ),
        (
            RedirectRejection::Policy {
                target: target.clone(),
                violation: Box::new(violation),
            },
            String::from("is disallowed"),
        ),
    ])
}

/// Every rejection is counted under a closed telemetry category of its own.
#[rstest]
fn every_rejection_has_a_distinct_closed_category() -> Result<()> {
    let categories = every_rejection()?
        .iter()
        .map(|(rejection, _message)| failure_category(rejection))
        .collect::<Vec<_>>();
    let unique = categories.iter().copied().collect::<BTreeSet<_>>();
    ensure!(
        unique.len() == categories.len(),
        "each rejection needs its own telemetry category: {categories:?}",
    );
    for category in categories {
        ensure!(
            telemetry::FETCH_REDIRECT_FAILURE_VALUES.contains(&category),
            "category {category} must come from the declared vocabulary",
        );
        ensure!(
            category != "none",
            "only a followed redirect may report the 'none' category",
        );
    }
    Ok(())
}

/// Every rejection renders its own localized diagnostic without credentials.
///
/// Fluent wraps each interpolated value in bidi isolate characters, so the
/// diagnostic is normalized before its wording is matched.
#[rstest]
fn every_rejection_renders_a_redacted_diagnostic() -> Result<()> {
    for (rejection, expected) in every_rejection()? {
        let rendered = normalize_fluent_isolates(&rejection_error(&rejection).to_string());
        ensure!(
            rendered.contains(expected.as_str()),
            "diagnostic for {rejection:?} should mention '{expected}', got {rendered}",
        );
        for secret in SECRETS {
            ensure!(
                !rendered.contains(secret),
                "diagnostic for {rejection:?} must not disclose {secret}: {rendered}",
            );
        }
    }
    Ok(())
}

/// Redaction keeps the location and drops the credentials.
#[rstest]
fn redacted_urls_keep_only_the_location() -> Result<()> {
    let redacted = redacted_url(&parse_url(CREDENTIALED_CURRENT)?);
    ensure!(
        redacted == "http://allowed.example/start",
        "redaction should keep only the location, got {redacted}",
    );
    Ok(())
}

/// An unexpired budget yields the time left, not a fresh per-hop timeout.
#[rstest]
fn remaining_budget_shrinks_towards_the_chain_deadline() -> Result<()> {
    let url = parse_url(CREDENTIALED_CURRENT)?;
    let deadline = Instant::now() + Duration::from_secs(30);
    let remaining = remaining_budget(deadline, &url)
        .context("an unexpired chain budget should yield the time remaining")?;
    ensure!(
        remaining > Duration::ZERO && remaining <= Duration::from_secs(30),
        "the remaining budget must shrink towards the deadline, got {remaining:?}",
    );
    Ok(())
}

/// An expired budget refuses the next hop instead of granting it more time.
#[rstest]
fn exhausted_budget_refuses_the_next_hop() -> Result<()> {
    let url = parse_url(CREDENTIALED_CURRENT)?;
    let Err(err) = remaining_budget(Instant::now(), &url) else {
        bail!("an expired chain budget must refuse the next hop");
    };
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "an expired budget should report InvalidOperation, got {:?}",
        err.kind(),
    );
    let rendered = err.to_string();
    ensure!(
        rendered.contains("exceeded its deadline"),
        "an expired budget should name the deadline, got {rendered}",
    );
    for secret in SECRETS {
        ensure!(
            !rendered.contains(secret),
            "an expired budget must not disclose {secret}: {rendered}",
        );
    }
    Ok(())
}

/// Only the statuses whose redirect semantics preserve GET are followed.
#[rstest]
#[case(300, false)]
#[case(301, true)]
#[case(302, true)]
#[case(303, true)]
#[case(304, false)]
#[case(307, true)]
#[case(308, true)]
fn supported_redirect_statuses_are_handled_explicitly(
    #[case] status: u16,
    #[case] should_redirect: bool,
) {
    assert_eq!(
        is_supported_redirect_status(status),
        should_redirect,
        "status {status} should be classified as a supported redirect: {should_redirect}"
    );
}

/// Every refusal renders the same diagnostic the user sees, snapshotted.
///
/// A substring check survives rewording, a dropped interpolation, or a leaked
/// location, so the whole normalized message is pinned instead. Each refusal
/// has its own category, which names its snapshot.
#[rstest]
fn every_rejection_diagnostic_is_snapshotted(en_localizer: EnLocalizer) -> Result<()> {
    let _localizer = en_localizer;

    for (rejection, _message) in every_rejection()? {
        let rendered = normalize_fluent_isolates(&rejection_error(&rejection).to_string());
        snapshot_settings("network_redirect").bind(|| {
            assert_snapshot!(failure_category(&rejection), rendered);
        });
    }
    Ok(())
}

/// The exhausted-budget diagnostic is snapshotted alongside the refusals.
#[rstest]
fn chain_deadline_diagnostic_is_snapshotted(en_localizer: EnLocalizer) -> Result<()> {
    let _localizer = en_localizer;
    let url = parse_url(CREDENTIALED_CURRENT)?;

    let Err(err) = remaining_budget(Instant::now(), &url) else {
        bail!("an expired chain budget must refuse the next hop");
    };
    let rendered = normalize_fluent_isolates(&err.to_string());
    snapshot_settings("network_redirect").bind(|| {
        assert_snapshot!("chain_deadline", rendered);
    });
    Ok(())
}

/// Build a credentialed URL for a loopback port with no listener.
///
/// Binding and then releasing the port gives the dispatch a connection the
/// kernel refuses at once, so the failure is deterministic and needs no
/// network or DNS.
///
/// # Errors
///
/// Returns an error when the probe listener cannot be bound or queried.
fn closed_loopback_url() -> Result<Url> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))
        .context("bind a probe listener for an unused port")?;
    let port = listener
        .local_addr()
        .context("read the probe listener address")?
        .port();
    drop(listener);
    parse_url(&format!(
        "http://redirect-user:redirect-secret@127.0.0.1:{port}/start"
    ))
}

/// A hop that cannot connect logs a bounded category and redacts the URL.
///
/// This is the only diagnostic the fixture-backed tests cannot reach: every
/// other failure is driven by a server that answers.
#[rstest]
fn failed_dispatch_reports_a_bounded_category(en_localizer: EnLocalizer) -> Result<()> {
    let _localizer = en_localizer;
    let unreachable = closed_loopback_url()?;
    let agent = build_redirect_agent();

    let (events, dispatch) = with_test_subscriber(LevelFilter::DEBUG, |captured| {
        // `ureq::Response` has no `Debug`, so the outcome is rendered here
        // rather than unwrapped with `expect_err`.
        let dispatch = dispatch_hop(&agent, &unreachable, Duration::from_secs(5))
            .map(|_response| ())
            .map_err(|err| err.to_string());
        (captured.snapshot(), dispatch)
    });

    let Err(rendered) = dispatch else {
        bail!("a refused connection must fail the hop");
    };
    ensure!(
        rendered.contains("HTTP request failed"),
        "a failed hop should name the transport failure, got {rendered}",
    );
    ensure!(
        rendered.contains("127.0.0.1"),
        "a failed hop should name the redacted location, got {rendered}",
    );
    for secret in SECRETS {
        ensure!(
            !rendered.contains(secret),
            "a failed hop must not disclose {secret}: {rendered}",
        );
    }
    ensure!(
        events
            .iter()
            .any(|event| event.contains("error_category=\"connection\"")
                && event.contains("host=\"127.0.0.1\"")),
        "a refused connection must log the bounded category, got {events:#?}",
    );
    Ok(())
}
