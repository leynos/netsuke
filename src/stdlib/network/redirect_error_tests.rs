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

use super::*;

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
/// The structured fixture renders only responses a real server could send, so
/// the raw-response fixture supplies the bytes instead. Its URL carries the
/// credentials this hop redacts, and its fixture drains the request before
/// writing and then half-closes, so the malformed bytes are received whole
/// rather than the read failing first.
///
/// # Errors
///
/// Returns an error when the fixture cannot be started or joined, the test URL
/// cannot be parsed, or the hop unexpectedly succeeds.
fn malformed_status_line_failure() -> Result<ureq::Error> {
    let (raw, _log, server) = http::spawn_http_server_raw_response(RawHttpResponse::new(
        b"HTTP/1.1 banana OK\r\nContent-Length: 0\r\n\r\n".as_slice(),
    ))
    .context("spawn a malformed-response server")?;

    let url = Url::parse(&format!("{raw}/start"))
        .with_context(|| format!("test URL should parse: {raw}"))?;
    let agent = build_redirect_agent();
    let outcome = request_hop(&agent, &url, Duration::from_secs(5));
    server
        .join()
        .map_err(|_panic| anyhow::anyhow!("malformed-response server panicked"))?;
    let Err(err) = outcome else {
        bail!("a malformed status line must fail the hop");
    };
    Ok(err)
}
