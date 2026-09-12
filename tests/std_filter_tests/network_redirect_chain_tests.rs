//! Integration tests for multi-hop fetch redirect chains.
//!
//! `network_redirect_tests` pins one observable behaviour per fixture. These
//! cases pin what a chain does *across* hops: the HTTP method that reaches each
//! hop of every supported redirect status, and the refusal of a chain whose
//! second destination the policy rejects. Both need the fixture's request log
//! or a second redirecting fixture, which the single-hop cases do not use.

use std::io;

use anyhow::{Context, Result, bail, ensure};
use netsuke::stdlib::NetworkPolicy;
use rstest::rstest;
use test_support::http::{self, HttpResponse};

use super::network_redirect_tests::{join_server, localhost_url, render_fetch};

/// Every supported redirect status is followed with GET at each hop.
#[rstest]
#[case(301)]
#[case(302)]
#[case(303)]
#[case(307)]
#[case(308)]
fn every_supported_redirect_status_is_followed_with_get(#[case] status: u16) -> Result<()> {
    let (url, log, server) = match http::spawn_http_server_recording([
        HttpResponse::new(status, "").with_header("Location", "/next"),
        HttpResponse::new(200, "redirected body"),
    ]) {
        Ok(fixture) => fixture,
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => return Ok(()),
        Err(err) => bail!("spawn {status} redirect fixture: {err}"),
    };
    let policy = NetworkPolicy::default().allow_scheme("http")?;

    let (rendered, impure) = render_fetch(policy, &url)?;
    join_server(server, "status redirect")?;
    ensure!(
        rendered == "redirected body",
        "status {status} should render the redirect target body",
    );
    ensure!(impure, "status {status} should mark the template impure");
    let lines = log.lines();
    ensure!(
        lines.len() == 2,
        "status {status} should issue exactly two requests: {lines:?}",
    );
    ensure!(
        lines.iter().all(|line| line.starts_with("GET ")),
        "status {status} must preserve GET at every hop: {lines:?}",
    );
    ensure!(
        lines
            .first()
            .is_some_and(|line| !line.contains("/next") && line.starts_with("GET / ")),
        "status {status} should request the original URL first: {lines:?}",
    );
    ensure!(
        lines.get(1).is_some_and(|line| line.contains("/next")),
        "status {status} should request the redirect target second: {lines:?}",
    );
    Ok(())
}

/// Assert that a chain follows one allowed hop and then refuses the next.
///
/// The entry fixture is served under `localhost` so it satisfies the allowlist,
/// the middle fixture answers with `middle_location`, and the denied fixture
/// must never be contacted. Every fixture is joined, so a chain that stopped
/// early is reported as a missing request rather than a hung server.
///
/// The denied fixture is the one fixture here whose request is not expected, so
/// it waits for its client without an accept deadline: the sole connection it
/// sees is the shutdown probe that joining it sends. Holding it to the accept
/// timeout failed Windows CI, where driving the chain took longer than the
/// timeout and the fixture panicked before the test could join it.
fn assert_second_hop_is_refused(
    policy: NetworkPolicy,
    middle_location: impl Fn(&str) -> Result<String>,
    expected_details: &str,
) -> Result<()> {
    let (denied_url, denied_log, denied_server) =
        match http::spawn_http_server_expecting_no_requests(HttpResponse::new(200, "denied target"))
        {
            Ok(fixture) => fixture,
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => return Ok(()),
            Err(err) => bail!("spawn refused hop fixture: {err}"),
        };
    let location = middle_location(&denied_url)?;
    let (middle_loopback, middle_log, middle_server) = http::spawn_http_server_recording([
        HttpResponse::new(302, "").with_header("Location", location),
    ])
    .context("spawn middle hop fixture")?;
    let middle_url = localhost_url(&middle_loopback)?;
    let (entry_loopback, entry_log, entry_server) = http::spawn_http_server_recording([
        HttpResponse::new(302, "").with_header("Location", middle_url),
    ])
    .context("spawn entry hop fixture")?;
    let entry_url = localhost_url(&entry_loopback)?;

    let err = match render_fetch(policy, &entry_url) {
        Ok(rendered) => bail!("refused redirect chain unexpectedly rendered: {rendered:?}"),
        Err(err) => err,
    };
    join_server(entry_server, "entry hop")?;
    join_server(middle_server, "middle hop")?;
    join_server(denied_server, "refused hop")?;

    ensure!(
        err.to_string().contains(expected_details),
        "the refused second hop should report '{expected_details}': {err}",
    );
    ensure!(
        entry_log.lines().len() == 1,
        "the entry hop should be requested exactly once: {:?}",
        entry_log.lines(),
    );
    ensure!(
        middle_log.lines().len() == 1,
        "the middle hop should be requested exactly once: {:?}",
        middle_log.lines(),
    );
    ensure!(
        denied_log.is_empty(),
        "the refused hop must receive no request: {:?}",
        denied_log.lines(),
    );
    Ok(())
}

/// Verify an allowed hop that redirects to a blocked host stops before it.
#[rstest]
fn allowed_hop_redirecting_to_blocked_host_is_refused() -> Result<()> {
    let policy = NetworkPolicy::default()
        .allow_scheme("http")?
        .deny_all_hosts()
        .allow_hosts(["localhost"])?
        .block_host("127.0.0.1")?;
    assert_second_hop_is_refused(
        policy,
        |denied| Ok(denied.to_owned()),
        "is blocked by policy",
    )
}

/// Verify an allowed hop that redirects off the allowlist stops before it.
#[rstest]
fn allowed_hop_redirecting_to_non_allowlisted_host_is_refused() -> Result<()> {
    let policy = NetworkPolicy::default()
        .allow_scheme("http")?
        .deny_all_hosts()
        .allow_hosts(["localhost"])?;
    assert_second_hop_is_refused(
        policy,
        |denied| Ok(denied.to_owned()),
        "is not on the allowlist",
    )
}

/// Verify an allowed hop that redirects to a disallowed scheme stops before it.
#[rstest]
fn allowed_hop_redirecting_to_disallowed_scheme_is_refused() -> Result<()> {
    let policy = NetworkPolicy::default().allow_scheme("http")?;
    assert_second_hop_is_refused(
        policy,
        |_denied| Ok(String::from("ftp://refused.example/next")),
        "is not allowed",
    )
}
