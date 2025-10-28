//! Integration tests for network functions.

use super::*;

use anyhow::{Context, Result, anyhow, ensure};
use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::stdlib::DEFAULT_FETCH_CACHE_DIR;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::value::{Kwargs, Value};
use rstest::{fixture, rstest};
use tempfile::tempdir;

#[fixture]
fn cache_workspace() -> Result<(tempfile::TempDir, Arc<Dir>, Utf8PathBuf)> {
    let temp = tempdir().context("create fetch cache tempdir")?;
    let temp_path = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow!("tempdir path not valid UTF-8: {path:?}"))?;
    let dir = Dir::open_ambient_dir(temp_path.as_path(), ambient_authority())
        .context("open cache workspace")?;
    Ok((temp, Arc::new(dir), temp_path))
}

fn make_context(root: Arc<Dir>) -> FetchContext {
    let config = NetworkConfig {
        cache_root: root,
        cache_relative: Utf8PathBuf::from(DEFAULT_FETCH_CACHE_DIR),
        policy: NetworkPolicy::default(),
    };
    FetchContext::new(config)
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
fn fetch_rejects_disallowed_scheme(cache_workspace: Result<CacheWorkspace>) -> Result<()> {
    let (_temp, root, _path) = cache_workspace?;
    let config = NetworkConfig {
        cache_root: root,
        cache_relative: Utf8PathBuf::from(DEFAULT_FETCH_CACHE_DIR),
        policy: NetworkPolicy::default(),
    };
    let context = FetchContext::new(config);
    let kwargs = std::iter::empty::<(String, Value)>().collect::<Kwargs>();
    let impure = Arc::new(AtomicBool::new(false));
    let Err(err) = fetch("http://example.com", &kwargs, &impure, &context) else {
        return Err(anyhow!("expected fetch to reject http scheme"));
    };
    ensure!(
        err.to_string().contains("scheme 'http' is not permitted"),
        "error should mention disallowed scheme: {err}",
    );
    ensure!(
        !impure.load(Ordering::Relaxed),
        "policy rejection must not mark the template impure",
    );
    Ok(())
}

#[rstest]
fn fetch_rejects_not_allowlisted_host(cache_workspace: Result<CacheWorkspace>) -> Result<()> {
    let (_temp, root, _path) = cache_workspace?;
    let config = NetworkConfig {
        cache_root: root,
        cache_relative: Utf8PathBuf::from(DEFAULT_FETCH_CACHE_DIR),
        policy: NetworkPolicy::default().deny_all_hosts(),
    };
    let context = FetchContext::new(config);
    let kwargs = std::iter::empty::<(String, Value)>().collect::<Kwargs>();
    let impure = Arc::new(AtomicBool::new(false));
    let Err(err) = fetch("https://example.com", &kwargs, &impure, &context) else {
        return Err(anyhow!("expected fetch to reject not-allowlisted host"));
    };
    ensure!(
        err.to_string().contains("not allowlisted"),
        "error should mention allowlist failure: {err}",
    );
    ensure!(
        !impure.load(Ordering::Relaxed),
        "policy rejection must not mark the template impure",
    );
    Ok(())
}

fn assert_open_cache_dir_rejects(root: &Dir, path: &Utf8Path, description: &str) -> Result<()> {
    let err = open_cache_dir(root, path).expect_err("open_cache_dir should reject invalid path");
    ensure!(
        err.to_string().contains(description),
        "error should mention {description}, got {err}",
    );
    Ok(())
}

type CacheWorkspace = (tempfile::TempDir, Arc<Dir>, Utf8PathBuf);
