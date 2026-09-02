//! Cache semantics tests for policy-checked fetch redirects.

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use test_support::http::{self, HttpResponse};
use url::Url;

use super::tests_support::{CacheWorkspace, cache_workspace, make_context_with};
use super::*;
use crate::stdlib::DEFAULT_FETCH_MAX_RESPONSE_BYTES;
use minijinja::value::{Kwargs, Value};

#[rstest]
fn fetch_caches_redirected_response_under_original_url(
    cache_workspace: Result<CacheWorkspace>,
) -> Result<()> {
    let (_temp, root, _workspace) = cache_workspace?;
    let (url, requests, server) = http::spawn_http_server_responses([
        HttpResponse::new(302, "").with_header("Location", "/cached"),
        HttpResponse::new(200, "redirected cache body"),
    ])
    .context("spawn redirect cache fixture")?;
    let policy = NetworkPolicy::default()
        .allow_scheme("http")
        .context("allow HTTP for redirect cache test")?;
    let context = make_context_with(root, policy, DEFAULT_FETCH_MAX_RESPONSE_BYTES);
    let kwargs = [(String::from("cache"), Value::from(true))]
        .into_iter()
        .collect::<Kwargs>();
    let impure = Arc::new(AtomicBool::new(false));

    let value = fetch(&url, &kwargs, &impure, &context)?;
    server
        .join()
        .map_err(|err| anyhow::anyhow!("redirect cache fixture panicked: {err:?}"))?;
    let cache_dir = context.open_cache_dir()?;
    let original_url = Url::parse(&url).context("parse original redirect URL")?;
    let original_key = cache_key(original_url.as_str());
    let redirected_url = original_url
        .join("/cached")
        .context("resolve redirect destination URL")?;
    let redirected_key = cache_key(redirected_url.as_str());

    ensure!(value.as_bytes() == Some(&b"redirected cache body"[..]));
    ensure!(
        read_cached(&cache_dir, &original_key, DEFAULT_FETCH_MAX_RESPONSE_BYTES)?
            == Some(b"redirected cache body".to_vec()),
        "redirected response should use the original URL cache key",
    );
    ensure!(
        read_cached(
            &cache_dir,
            &redirected_key,
            DEFAULT_FETCH_MAX_RESPONSE_BYTES
        )?
        .is_none(),
        "redirect destination must not become a second cache key",
    );
    ensure!(impure.load(Ordering::Relaxed));
    ensure!(requests.load(Ordering::Relaxed) == 2);
    Ok(())
}
