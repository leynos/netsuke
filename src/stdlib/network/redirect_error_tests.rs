//! Unit tests for the redirect adapter's `error_category` classification.
//!
//! Every failure of a hop is logged with a category from a closed vocabulary,
//! which is the only field that separates a refused connection from a timeout,
//! a malformed response, and an unusable URL. These cases pin the mapping from
//! `ureq`'s failure variants onto that vocabulary, so a client upgrade that
//! renames or adds a variant is caught here rather than by a silent fallback to
//! `other`.

use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use rstest::rstest;
use test_support::http::{self, RawHttpResponse};

use super::super::tests_support::credentialed_loopback_url;
use super::*;

/// A status line no HTTP client accepts, followed by a well-formed end of head.
///
/// The header block is complete so a client that reaches the parser fails on the
/// status code itself rather than on a truncated response, which keeps the case
/// about classification instead of framing.
const MALFORMED_STATUS_LINE: &str = "HTTP/1.1 banana OK\r\nContent-Length: 0\r\n\r\n";

/// Every `ureq` failure maps into the closed `error_category` vocabulary.
///
/// The category separates an unsuccessful HTTP response from a connection, a
/// timeout, a malformed response, and an unusable URL, so each case is built
/// from a stable variant carrying that cause. `Protocol` is absent because it
/// wraps a parser error type this crate does not name, and is covered by
/// [`protocol_failures_are_classified_from_a_live_response`] instead.
#[rstest]
#[case::http_status(ureq::Error::StatusCode(404), "http_status")]
#[case::timeout(ureq::Error::Timeout(ureq::Timeout::Global), "timeout")]
#[case::host_not_found(ureq::Error::HostNotFound, "connection")]
#[case::connection_failed(ureq::Error::ConnectionFailed, "connection")]
#[case::connect_proxy_failed(
    ureq::Error::ConnectProxyFailed(String::from("proxy refused")),
    "connection"
)]
#[case::timed_out_io(
    ureq::Error::Io(std::io::Error::from(std::io::ErrorKind::TimedOut)),
    "timeout"
)]
#[case::refused_io(
    ureq::Error::Io(std::io::Error::from(std::io::ErrorKind::ConnectionRefused)),
    "connection"
)]
#[case::reset_io(
    ureq::Error::Io(std::io::Error::from(std::io::ErrorKind::ConnectionReset)),
    "connection"
)]
#[case::aborted_io(
    ureq::Error::Io(std::io::Error::from(std::io::ErrorKind::ConnectionAborted)),
    "connection"
)]
#[case::ordinary_io(
    ureq::Error::Io(std::io::Error::from(std::io::ErrorKind::BrokenPipe)),
    "io"
)]
#[case::invalid_uri(
    ureq::Error::BadUri(String::from("mailto:nobody@example.invalid")),
    "invalid_url"
)]
#[case::invalid_proxy_url(ureq::Error::InvalidProxyUrl, "invalid_url")]
#[case::require_https_only(
    ureq::Error::RequireHttpsOnly(String::from("http://allowed.example/")),
    "invalid_url"
)]
#[case::unclassified(ureq::Error::TooManyRedirects, "other")]
#[case::redirect_failed(ureq::Error::RedirectFailed, "other")]
fn every_ureq_failure_maps_to_a_closed_category(#[case] err: ureq::Error, #[case] expected: &str) {
    assert_eq!(
        ureq_failure_category(&err),
        expected,
        "{err} should classify as {expected}",
    );
}

/// A response that cannot be parsed still classifies as `protocol`.
///
/// `ureq::Error::Protocol` wraps a parser error whose payload this crate cannot
/// name, so the variant itself is asserted here and its category is covered
/// while the live case below confirms the production path reaches it.
#[rstest]
fn protocol_failures_are_classified_from_a_live_response() -> Result<()> {
    let err = malformed_status_line_failure()?;
    ensure!(
        matches!(err, ureq::Error::Protocol(_)),
        "a malformed status line should surface as a protocol error, got {err}",
    );
    ensure!(
        ureq_failure_category(&err) == "protocol",
        "a malformed status line should classify as protocol, got {}",
        ureq_failure_category(&err),
    );
    Ok(())
}

/// Drive a hop against a server whose status line cannot be parsed.
///
/// The raw-response fixture emits the malformed bytes and then completes the
/// response with a write-side shutdown, so the client reads exactly this payload
/// instead of racing the fixture's teardown. A bare listener that closed instead
/// raced the client: on Windows the close reset the connection and the client
/// reported an aborted connection in place of the parse failure, which is the
/// defect this case exists to catch. Nothing here is platform-gated, because
/// with the response completed properly both platforms reach the parser.
///
/// # Errors
///
/// Returns an error when the fixture cannot be started or joined, when the test
/// URL cannot be built, or when the hop unexpectedly succeeds.
fn malformed_status_line_failure() -> Result<ureq::Error> {
    let (fixture_url, _requests, server) =
        http::spawn_raw_http_server(RawHttpResponse::text(MALFORMED_STATUS_LINE))
            .context("spawn a malformed-response fixture")?;
    let url = credentialed_loopback_url(&fixture_url)?;
    let agent = build_redirect_agent();

    let outcome = request_hop(&agent, &url, Duration::from_secs(5));
    server
        .join()
        .map_err(|_panic| anyhow::anyhow!("malformed-response fixture panicked"))?;
    let Err(err) = outcome else {
        bail!("a malformed status line must fail the hop");
    };
    Ok(err)
}
