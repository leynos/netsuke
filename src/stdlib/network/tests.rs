//! Integration tests for network functions.

use super::*;

use anyhow::{Context, Result, anyhow, ensure};
use std::{
    convert::TryFrom,
    fs,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::stdlib::{DEFAULT_FETCH_CACHE_DIR, DEFAULT_FETCH_MAX_RESPONSE_BYTES};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{
    ErrorKind,
    value::{Kwargs, Value},
};
use rstest::{fixture, rstest};
use tempfile::tempdir;
use test_support::http;
use url::Url;

/// Creates a temporary cache workspace returning the tempdir, an ambient
/// authority directory handle wrapped in `Arc`, and the UTF-8 path for cache
/// assertions in fetch tests.
#[fixture]
fn cache_workspace() -> Result<(tempfile::TempDir, Arc<Dir>, Utf8PathBuf)> {
    let temp = tempdir().context("create fetch cache tempdir")?;
    let temp_path = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow!("tempdir path not valid UTF-8: {path:?}"))?;
    let dir = Dir::open_ambient_dir(temp_path.as_path(), ambient_authority())
        .context("open cache workspace")?;
    Ok((temp, Arc::new(dir), temp_path))
}

/// Builds a test `FetchContext` with the provided cache root and default policy.
fn make_context(root: Arc<Dir>) -> FetchContext {
    make_context_with(
        root,
        NetworkPolicy::default(),
        DEFAULT_FETCH_MAX_RESPONSE_BYTES,
    )
}

fn make_context_with(root: Arc<Dir>, policy: NetworkPolicy, limit: u64) -> FetchContext {
    let config = NetworkConfig {
        cache_root: root,
        cache_relative: Utf8PathBuf::from(DEFAULT_FETCH_CACHE_DIR),
        policy,
        max_response_bytes: limit,
    };
    FetchContext::new(config)
}

fn limit_with_offset(limit: u64, offset: u64) -> usize {
    let total = limit
        .checked_add(offset)
        .expect("test limit plus offset should not overflow");
    usize::try_from(total).expect("test limit plus offset should fit into usize")
}

#[rstest]
fn cache_key_hashes_url() {
    let digest = cache_key("https://example.com");
    assert_eq!(digest.len(), 64);
}

#[rstest]
fn hex_string_encodes_bytes() {
    assert_eq!(hex_string(&[0xde, 0xad, 0xbe, 0xef]), "deadbeef");
}

#[rstest]
fn to_value_returns_bytes_for_invalid_utf8() {
    let value = value_from_bytes(vec![0xff, 0xfe, 0xfd]);
    assert_eq!(value.as_bytes(), Some(&[0xff, 0xfe, 0xfd][..]));
}

#[rstest]
#[case("", "must not be empty")]
#[case("/etc/netsuke-cache", "must be relative to the workspace")]
#[case("../escape", "must stay within the workspace")]
fn open_cache_dir_rejects_invalid_paths(
    cache_workspace: Result<CacheWorkspace>,
    #[case] path: &str,
    #[case] description: &str,
) -> Result<()> {
    let (_temp, root, _path) = cache_workspace?;
    assert_open_cache_dir_rejects(root.as_ref(), Utf8Path::new(path), description)
}

/// Write an entry to the cache directory and assert it exists within the workspace.
fn assert_cache_entry_exists(
    dir: Dir,
    cache_relative: &Utf8Path,
    workspace: &Utf8Path,
    entry_name: &str,
) -> Result<()> {
    dir.write(entry_name, b"data")
        .context("write cache entry")?;
    drop(dir);
    let entry = workspace.join(cache_relative).join(entry_name);
    ensure!(
        fs::metadata(entry.as_std_path()).is_ok(),
        "entry {} should exist",
        entry
    );
    Ok(())
}

#[rstest]
fn open_cache_dir_errors_for_file_path(cache_workspace: Result<CacheWorkspace>) -> Result<()> {
    let (_temp, root, path) = cache_workspace?;
    let file_path = path.join("file");
    fs::write(file_path.as_std_path(), b"data").context("write file placeholder")?;
    assert_open_cache_dir_rejects(
        root.as_ref(),
        file_path.as_path(),
        "must be relative to the workspace",
    )
}

#[rstest]
fn open_cache_dir_creates_relative_directory(
    cache_workspace: Result<CacheWorkspace>,
) -> Result<()> {
    let (_temp, root, path) = cache_workspace?;
    let dir = open_cache_dir(&root, Utf8Path::new("cache"))?;
    assert_cache_entry_exists(dir, Utf8Path::new("cache"), path.as_path(), "entry")
}

#[rstest]
fn fetch_rejects_template_cache_dir_argument(
    cache_workspace: Result<CacheWorkspace>,
) -> Result<()> {
    let (_temp, root, _path) = cache_workspace?;
    let context = make_context(root);
    let kwargs = [(String::from("cache_dir"), Value::from(".netsuke/cache"))]
        .into_iter()
        .collect::<Kwargs>();
    let impure = Arc::new(AtomicBool::new(false));
    let Err(err) = fetch("http://127.0.0.1:9", &kwargs, &impure, &context) else {
        return Err(anyhow!(
            "expected cache_dir keyword to fail but request succeeded"
        ));
    };
    ensure!(
        err.kind() == ErrorKind::TooManyArguments,
        "unexpected error kind {kind:?}",
        kind = err.kind()
    );
    ensure!(
        err.to_string().contains("cache_dir"),
        "error should mention unexpected cache_dir argument: {err}",
    );
    ensure!(
        !impure.load(Ordering::Relaxed),
        "rejecting cache_dir must not mark the template impure",
    );
    Ok(())
}

#[rstest]
fn fetch_cache_opens_default_directory(cache_workspace: Result<CacheWorkspace>) -> Result<()> {
    let (_temp, root, path) = cache_workspace?;
    let context = make_context(root);
    let dir = context.open_cache_dir()?;
    assert_cache_entry_exists(
        dir,
        Utf8Path::new(DEFAULT_FETCH_CACHE_DIR),
        path.as_path(),
        "entry",
    )
}

#[rstest]
fn fetch_rejects_responses_over_the_limit(cache_workspace: Result<CacheWorkspace>) -> Result<()> {
    let (_temp, root, _path) = cache_workspace?;
    let limit = 16_u64;
    let body = "x".repeat(limit_with_offset(limit, 1));
    let (url, _server) =
        http::spawn_http_server(body).context("spawn HTTP server for oversized response test")?;
    let policy = NetworkPolicy::default()
        .allow_scheme("http")
        .context("allow http scheme for oversized response test")?;
    let context = make_context_with(root, policy, limit);
    let kwargs = std::iter::empty::<(String, Value)>().collect::<Kwargs>();
    let impure = Arc::new(AtomicBool::new(false));
    let Err(err) = fetch(&url, &kwargs, &impure, &context) else {
        return Err(anyhow!("expected fetch to reject response exceeding limit"));
    };
    ensure!(
        err.to_string().contains("configured limit of 16 bytes"),
        "error should describe limit: {err}",
    );
    ensure!(
        impure.load(Ordering::Relaxed),
        "oversized network response should mark the template impure",
    );
    Ok(())
}

#[rstest]
fn fetch_streams_responses_into_cache(cache_workspace: Result<CacheWorkspace>) -> Result<()> {
    let (_temp, root, workspace) = cache_workspace?;
    let policy = NetworkPolicy::default()
        .allow_scheme("http")
        .context("allow http scheme for cache streaming test")?;
    let limit = 1024_u64;
    let body = "cached response";
    let (url, _server) =
        http::spawn_http_server(body).context("spawn HTTP server for cache streaming test")?;
    let context = make_context_with(Arc::clone(&root), policy, limit);
    let kwargs = [(String::from("cache"), Value::from(true))]
        .into_iter()
        .collect::<Kwargs>();
    let impure = Arc::new(AtomicBool::new(false));
    let value = fetch(&url, &kwargs, &impure, &context)?;
    ensure!(
        value.as_bytes() == Some(body.as_bytes()),
        "fetch should return response body",
    );
    ensure!(
        impure.load(Ordering::Relaxed),
        "successful fetch should mark template impure",
    );
    let cache_dir = context.open_cache_dir()?;
    let parsed = Url::parse(&url).context("parse cache URL")?;
    let key = cache_key(parsed.as_str());
    let cached =
        read_cached(&cache_dir, &key, limit)?.context("cached response should be present")?;
    ensure!(
        cached == body.as_bytes(),
        "cache contents should match response"
    );
    let cache_path = workspace
        .join(DEFAULT_FETCH_CACHE_DIR)
        .join(Utf8Path::new(&key));
    ensure!(
        fs::metadata(cache_path.as_std_path()).is_ok(),
        "cache entry should be written to disk",
    );
    Ok(())
}

#[rstest]
fn fetch_clears_partial_cache_on_limit_error(
    cache_workspace: Result<CacheWorkspace>,
) -> Result<()> {
    let (_temp, root, workspace) = cache_workspace?;
    let policy = NetworkPolicy::default()
        .allow_scheme("http")
        .context("allow http scheme for cache failure test")?;
    let limit = 32_u64;
    let body = "y".repeat(limit_with_offset(limit, 8));
    let (url, _server) =
        http::spawn_http_server(body).context("spawn HTTP server for cache failure test")?;
    let context = make_context_with(Arc::clone(&root), policy, limit);
    let kwargs = [(String::from("cache"), Value::from(true))]
        .into_iter()
        .collect::<Kwargs>();
    let impure = Arc::new(AtomicBool::new(false));
    let parsed = Url::parse(&url).context("parse cache URL for failure test")?;
    let key = cache_key(parsed.as_str());
    let Err(err) = fetch(&url, &kwargs, &impure, &context) else {
        return Err(anyhow!(
            "expected fetch to reject oversized cached response"
        ));
    };
    ensure!(
        err.to_string().contains("configured limit"),
        "limit error should mention configured limit: {err}",
    );
    ensure!(
        impure.load(Ordering::Relaxed),
        "failed fetch should mark template impure",
    );
    let cache_dir = context.open_cache_dir()?;
    ensure!(
        cache_dir
            .open(Utf8Path::new(&key))
            .expect_err("oversized fetch should not leave cache entry")
            .kind()
            == io::ErrorKind::NotFound,
        "partial cache file should be removed",
    );
    let cache_path = workspace
        .join(DEFAULT_FETCH_CACHE_DIR)
        .join(Utf8Path::new(&key));
    ensure!(
        fs::metadata(cache_path.as_std_path()).is_err(),
        "no cache file should remain on disk",
    );
    Ok(())
}

#[rstest]
fn fetch_rejects_cached_entries_exceeding_limit(
    cache_workspace: Result<CacheWorkspace>,
) -> Result<()> {
    let (_temp, root, workspace) = cache_workspace?;
    let policy = NetworkPolicy::default()
        .allow_scheme("http")
        .context("allow http scheme for cached entry limit test")?;
    let limit = 24_u64;
    let (url, _server) = http::spawn_http_server("cached")
        .context("spawn HTTP server for cached entry limit test")?;
    let context = make_context_with(Arc::clone(&root), policy, limit);
    let cache_dir = context.open_cache_dir()?;
    let parsed = Url::parse(&url).context("parse cache URL for oversized entry test")?;
    let key = cache_key(parsed.as_str());
    let oversized = "z".repeat(limit_with_offset(limit, 1));
    cache_dir
        .write(Utf8Path::new(&key), oversized.as_bytes())
        .context("seed oversized cache entry")?;
    let kwargs = [(String::from("cache"), Value::from(true))]
        .into_iter()
        .collect::<Kwargs>();
    let impure = Arc::new(AtomicBool::new(false));
    let Err(err) = fetch(&url, &kwargs, &impure, &context) else {
        return Err(anyhow!("expected fetch to reject oversized cache entry"));
    };
    ensure!(
        err.to_string()
            .contains("exceeded the configured fetch limit of 24 bytes"),
        "error should mention cached entry limit: {err}",
    );
    ensure!(
        !impure.load(Ordering::Relaxed),
        "cache limit failure should not mark the template impure",
    );
    let cache_path = workspace
        .join(DEFAULT_FETCH_CACHE_DIR)
        .join(Utf8Path::new(&key));
    ensure!(
        fs::metadata(cache_path.as_std_path()).is_ok(),
        "existing cache entry should remain for investigation",
    );
    Ok(())
}

#[rstest]
fn fetch_rejects_disallowed_scheme(cache_workspace: Result<CacheWorkspace>) -> Result<()> {
    let (_temp, root, _path) = cache_workspace?;
    assert_fetch_policy_rejection(
        root,
        NetworkPolicy::default(),
        "http://example.com",
        "scheme 'http' is not permitted",
    )
}

#[rstest]
fn fetch_rejects_not_allowlisted_host(cache_workspace: Result<CacheWorkspace>) -> Result<()> {
    let (_temp, root, _path) = cache_workspace?;
    assert_fetch_policy_rejection(
        root,
        NetworkPolicy::default().deny_all_hosts(),
        "https://example.com",
        "not allowlisted",
    )
}

/// Asserts that `open_cache_dir` rejects the `path` with an error message containing `description`.
fn assert_open_cache_dir_rejects(root: &Dir, path: &Utf8Path, description: &str) -> Result<()> {
    let err = open_cache_dir(root, path).expect_err("open_cache_dir should reject invalid path");
    ensure!(
        err.to_string().contains(description),
        "error should mention {description}, got {err}",
    );
    Ok(())
}

/// Asserts that `fetch` rejects `url` under `policy` without marking the template impure.
fn assert_fetch_policy_rejection(
    root: Arc<Dir>,
    policy: NetworkPolicy,
    url: &str,
    expected_substring: &str,
) -> Result<()> {
    let config = NetworkConfig {
        cache_root: root,
        cache_relative: Utf8PathBuf::from(DEFAULT_FETCH_CACHE_DIR),
        policy,
        max_response_bytes: DEFAULT_FETCH_MAX_RESPONSE_BYTES,
    };
    let context = FetchContext::new(config);
    let kwargs = std::iter::empty::<(String, Value)>().collect::<Kwargs>();
    let impure = Arc::new(AtomicBool::new(false));
    let Err(err) = fetch(url, &kwargs, &impure, &context) else {
        return Err(anyhow!("expected fetch to reject '{url}'"));
    };
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "fetch should report InvalidOperation on policy rejection but was {:?}",
        err.kind(),
    );
    ensure!(
        err.to_string().contains(expected_substring),
        "error should mention expected substring '{expected_substring}': {err}",
    );
    ensure!(
        !impure.load(Ordering::Relaxed),
        "policy rejection must not mark the template impure",
    );
    Ok(())
}

type CacheWorkspace = (tempfile::TempDir, Arc<Dir>, Utf8PathBuf);
