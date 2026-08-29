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

use super::tests_support::{CacheWorkspace, cache_workspace, make_context, make_context_with};
use crate::stdlib::DEFAULT_FETCH_MAX_RESPONSE_BYTES;
use minijinja::value::{Kwargs, Value};

/// Verify fetch emits bounded allowed and rejected policy decisions.
#[rstest]
fn fetch_records_bounded_policy_decisions(cache_workspace: Result<CacheWorkspace>) -> Result<()> {
    let (_temp, root, _path) = cache_workspace?;
    let (url, _server) =
        http::spawn_http_server("policy allowed").context("spawn HTTP server for policy trace")?;
    let allowed_policy = NetworkPolicy::default()
        .allow_scheme("http")
        .context("allow HTTP for policy trace")?;
    let allowed_context = make_context_with(
        Arc::clone(&root),
        allowed_policy,
        DEFAULT_FETCH_MAX_RESPONSE_BYTES,
    );
    let rejected_context = make_context(root);
    let kwargs = std::iter::empty::<(String, Value)>().collect::<Kwargs>();
    let allowed_impure = Arc::new(AtomicBool::new(false));
    let rejected_impure = Arc::new(AtomicBool::new(false));

    let events = with_test_subscriber(LevelFilter::DEBUG, |captured| {
        fetch(&url, &kwargs, &allowed_impure, &allowed_context)
            .context("allow local HTTP fetch after policy evaluation")?;
        fetch(
            "http://example.test",
            &kwargs,
            &rejected_impure,
            &rejected_context,
        )
        .expect_err("default HTTPS-only policy should reject HTTP");
        Ok::<_, anyhow::Error>(captured.snapshot())
    })?;

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
                && event.contains("policy_reason=\"scheme_not_allowed\"")),
        "expected a bounded rejected fetch-policy event, got {events:#?}",
    );
    ensure!(
        !events.iter().any(|event| event.contains("example.test")),
        "policy decision events must not disclose raw URLs or hosts: {events:#?}",
    );
    ensure!(
        allowed_impure.load(Ordering::Relaxed),
        "allowed fetch should mark its template impure",
    );
    ensure!(
        !rejected_impure.load(Ordering::Relaxed),
        "rejected fetch must not mark its template impure",
    );
    Ok(())
}
