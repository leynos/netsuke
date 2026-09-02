//! Observability tests for network-policy decisions at the fetch boundary.

use super::*;

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use test_support::{http, tracing_capture::with_test_subscriber};
use tracing_subscriber::filter::LevelFilter;

use super::tests_support::{CacheWorkspace, cache_workspace, make_context_with};
use crate::stdlib::DEFAULT_FETCH_MAX_RESPONSE_BYTES;
use minijinja::value::{Kwargs, Value};

/// Verify fetch emits bounded allowed and rejected policy decisions.
#[rstest]
fn fetch_records_bounded_policy_decisions(cache_workspace: Result<CacheWorkspace>) -> Result<()> {
    let (_temp, root, _path) = cache_workspace?;
    let (url, allowed_server) =
        http::spawn_http_server("policy allowed").context("spawn HTTP server for policy trace")?;
    let (redirector_url, _redirector_requests, redirector_server) =
        http::spawn_http_server_responses([http::HttpResponse::new(302, "").with_header(
            "Location",
            "http://redirect-user:redirect-secret@blocked.example/",
        )])
        .context("spawn HTTP redirector for policy trace")?;
    let allowed_policy = NetworkPolicy::default()
        .allow_scheme("http")
        .context("allow HTTP for policy trace")?;
    let allowed_context = make_context_with(
        Arc::clone(&root),
        allowed_policy,
        DEFAULT_FETCH_MAX_RESPONSE_BYTES,
    );
    let rejected_policy = NetworkPolicy::default()
        .allow_scheme("http")
        .context("allow HTTP for rejected redirect trace")?
        .deny_all_hosts()
        .allow_hosts(["127.0.0.1"])
        .context("allow redirector host for policy trace")?;
    let rejected_context =
        make_context_with(root, rejected_policy, DEFAULT_FETCH_MAX_RESPONSE_BYTES);
    let kwargs = std::iter::empty::<(String, Value)>().collect::<Kwargs>();
    let allowed_impure = Arc::new(AtomicBool::new(false));
    let rejected_impure = Arc::new(AtomicBool::new(false));

    let events = with_test_subscriber(LevelFilter::DEBUG, |captured| {
        fetch(&url, &kwargs, &allowed_impure, &allowed_context)
            .context("allow local HTTP fetch after policy evaluation")?;
        fetch(
            &redirector_url,
            &kwargs,
            &rejected_impure,
            &rejected_context,
        )
        .expect_err("redirect target outside the allowlist should be rejected");
        Ok::<_, anyhow::Error>(captured.snapshot())
    })?;
    allowed_server
        .join()
        .map_err(|err| anyhow::anyhow!("allowed server thread panicked: {err:?}"))?;
    redirector_server
        .join()
        .map_err(|err| anyhow::anyhow!("redirector server thread panicked: {err:?}"))?;

    ensure!(
        events
            .iter()
            .any(|event| event.contains("operation=\"fetch\"")
                && event.contains("policy_outcome=\"allowed\"")),
        "expected a bounded allowed fetch-policy event, got {events:#?}",
    );
    ensure!(
        events
            .iter()
            .any(|event| event.contains("operation=\"fetch\"")
                && event.contains("policy_outcome=\"rejected\"")
                && event.contains("policy_reason=\"host_not_allowlisted\"")
                && event.contains("hop=1")),
        "expected a bounded rejected redirect-policy event, got {events:#?}",
    );
    ensure!(
        !events.iter().any(|event| event.contains("blocked.example")
            || event.contains("redirect-user")
            || event.contains("redirect-secret")),
        "redirect policy events must not disclose raw hosts or userinfo: {events:#?}",
    );
    ensure!(
        allowed_impure.load(Ordering::Relaxed),
        "allowed fetch should mark its template impure",
    );
    ensure!(
        rejected_impure.load(Ordering::Relaxed),
        "redirect rejection after the initial request must mark the template impure",
    );
    Ok(())
}
