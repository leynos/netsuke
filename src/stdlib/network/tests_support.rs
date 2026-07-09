//! Shared fixtures and assertion helpers for the network fetch tests.
//!
//! Provides the temporary cache workspace fixture, `FetchContext` builders,
//! and reusable assertions for cache-directory and policy rejections.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use anyhow::{Context, Result, anyhow, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{
    ErrorKind,
    value::{Kwargs, Value},
};
use rstest::fixture;
use tempfile::tempdir;
use test_support::fs;

use super::super::{FetchContext, NetworkConfig, NetworkPolicy, fetch, open_cache_dir};
use crate::localization;
use crate::stdlib::{DEFAULT_FETCH_CACHE_DIR, DEFAULT_FETCH_MAX_RESPONSE_BYTES};

pub(super) type CacheWorkspace = (tempfile::TempDir, Arc<Dir>, Utf8PathBuf);

/// Creates a temporary cache workspace returning the tempdir, an ambient
/// authority directory handle wrapped in `Arc`, and the UTF-8 path for cache
/// assertions in fetch tests.
#[fixture]
pub(super) fn cache_workspace() -> Result<CacheWorkspace> {
    let temp = tempdir().context("create fetch cache tempdir")?;
    let temp_path = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow!("tempdir path not valid UTF-8: {path:?}"))?;
    let dir = Dir::open_ambient_dir(temp_path.as_path(), ambient_authority())
        .context("open cache workspace")?;
    Ok((temp, Arc::new(dir), temp_path))
}

/// Builds a test `FetchContext` with the provided cache root and default policy.
pub(super) fn make_context(root: Arc<Dir>) -> FetchContext {
    make_context_with(
        root,
        NetworkPolicy::default(),
        DEFAULT_FETCH_MAX_RESPONSE_BYTES,
    )
}

pub(super) fn make_context_with(root: Arc<Dir>, policy: NetworkPolicy, limit: u64) -> FetchContext {
    let config = NetworkConfig {
        cache_root: root,
        cache_relative: Utf8PathBuf::from(DEFAULT_FETCH_CACHE_DIR),
        policy,
        max_response_bytes: limit,
    };
    FetchContext::new(config)
}

/// Computes `limit + offset` as a `usize` for oversized-response fixtures.
pub(super) fn limit_with_offset(limit: u64, offset: u64) -> Result<usize> {
    let total = limit
        .checked_add(offset)
        .context("test limit plus offset should not overflow")?;
    usize::try_from(total).context("test limit plus offset should fit into usize")
}

/// Write an entry to the cache directory and assert it exists within the workspace.
pub(super) fn assert_cache_entry_exists(
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
        fs::exists(entry.as_std_path()),
        "entry {} should exist",
        entry
    );
    Ok(())
}

/// Asserts that `open_cache_dir` rejects the `path` with an error message containing `expected`.
pub(super) fn assert_open_cache_dir_rejects(
    root: &Dir,
    path: &Utf8Path,
    expected: &str,
) -> Result<()> {
    let err = open_cache_dir(root, path).expect_err("open_cache_dir should reject invalid path");
    ensure!(
        err.to_string().contains(expected),
        "error should mention {expected}, got {err}",
    );
    Ok(())
}

/// Asserts that `fetch` rejects `url` under `policy` without marking the template impure.
pub(super) fn assert_fetch_policy_rejection(
    root: Arc<Dir>,
    policy: NetworkPolicy,
    url: &str,
    expected_message: &str,
) -> Result<()> {
    let context = make_context_with(root, policy, DEFAULT_FETCH_MAX_RESPONSE_BYTES);
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
        err.to_string().contains(expected_message),
        "error should mention expected message '{expected_message}': {err}",
    );
    ensure!(
        !impure.load(Ordering::Relaxed),
        "policy rejection must not mark the template impure",
    );
    Ok(())
}

pub(super) fn cache_relative_error(key: &'static str, path: Option<&str>) -> String {
    let message = path.map_or_else(
        || localization::message(key),
        |value| localization::message(key).with_arg("path", value),
    );
    message.to_string()
}
