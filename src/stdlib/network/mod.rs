//! Network helpers exposed to `MiniJinja` templates.
//!
//! This module wires the high-level `fetch` helper that retrieves remote
//! resources with optional on-disk caching. Request validation lives in the
//! sibling [`policy`] module so the I/O-focused code here stays small and
//! explicit.

mod policy;

pub use self::policy::{NetworkPolicy, NetworkPolicyViolation};

use std::{
    io::{self, Read},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use super::{NetworkConfig, StdlibConfig, value_from_bytes};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::Dir;
use minijinja::{
    Environment, Error, ErrorKind,
    value::{Kwargs, Value},
};
use sha2::{Digest, Sha256};
use url::Url;

/// Register network functions into the template environment.
///
/// Currently registers the `fetch` function for retrieving remote resources
/// with optional on-disk caching.
///
/// # Parameters
///
/// * `env` - `MiniJinja` environment to register functions into.
/// * `impure` - Shared flag for tracking template impurity.
pub(crate) fn register_functions(
    env: &mut Environment<'_>,
    impure: Arc<AtomicBool>,
    config: NetworkConfig,
) {
    let context = Arc::new(FetchContext::new(config));
    env.add_function("fetch", move |url: String, kwargs: Kwargs| {
        fetch(&url, &kwargs, &impure, &context)
    });
}

fn fetch(
    url: &str,
    kwargs: &Kwargs,
    impure: &Arc<AtomicBool>,
    context: &FetchContext,
) -> Result<Value, Error> {
    let use_cache = kwargs.get::<Option<bool>>("cache")?.unwrap_or(false);
    kwargs.assert_all_used()?;

    let parsed = Url::parse(url).map_err(|err| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("fetch URL '{url}' is invalid: {err}"),
        )
    })?;

    context.policy().evaluate(&parsed).map_err(|violation| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("fetch disallowed for '{url}': {violation}"),
        )
    })?;

    let bytes = if use_cache {
        let dir = context.open_cache_dir()?;
        let key = cache_key(url);
        if let Some(cached) = read_cached(&dir, &key)? {
            impure.store(true, Ordering::Relaxed);
            cached
        } else {
            let data = fetch_remote(&parsed, impure)?;
            write_cache(&dir, &key, &data, impure)?;
            data
        }
    } else {
        fetch_remote(&parsed, impure)?
    };

    Ok(value_from_bytes(bytes))
}

fn fetch_remote(url: &Url, impure: &Arc<AtomicBool>) -> Result<Vec<u8>, Error> {
    impure.store(true, Ordering::Relaxed);
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .timeout_write(Duration::from_secs(30))
        .timeout(Duration::from_secs(60))
        .build();
    let response = agent.get(url.as_str()).call().map_err(|err| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("fetch failed for '{}': {err}", url.as_str()),
        )
    })?;
    let mut reader = response.into_reader();
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).map_err(|err| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("failed to read response from '{}': {err}", url.as_str()),
        )
    })?;
    Ok(bytes)
}

fn open_cache_dir(root: &Dir, relative: &Utf8Path) -> Result<Dir, Error> {
    if let Err(err) = StdlibConfig::validate_cache_relative(relative) {
        return Err(Error::new(ErrorKind::InvalidOperation, err.to_string()));
    }

    root.create_dir_all(relative)
        .map_err(|err| io_error("create cache dir", relative, &err))?;
    root.open_dir(relative)
        .map_err(|err| io_error("open cache dir", relative, &err))
}

fn read_cached(dir: &Dir, name: &str) -> Result<Option<Vec<u8>>, Error> {
    match dir.open(name) {
        Ok(mut file) => {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf).map_err(|err| {
                Error::new(
                    ErrorKind::InvalidOperation,
                    format!("failed to read cache entry '{name}': {err}"),
                )
            })?;
            Ok(Some(buf))
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("failed to open cache entry '{name}': {err}"),
        )),
    }
}

fn write_cache(dir: &Dir, name: &str, data: &[u8], impure: &Arc<AtomicBool>) -> Result<(), Error> {
    impure.store(true, Ordering::Relaxed);
    dir.write(name, data).map_err(|err| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("failed to write cache entry '{name}': {err}"),
        )
    })
}

fn cache_key(url: &str) -> String {
    let digest = Sha256::digest(url.as_bytes());
    hex_string(&digest)
}

fn hex_string(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let result = write!(out, "{byte:02x}");
        #[expect(
            clippy::expect_used,
            reason = "hex output to String should be infallible"
        )]
        result.expect("writing hex to String never fails");
    }
    out
}

fn io_error(action: &str, path: &Utf8Path, err: &io::Error) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!("{action} for '{path}' failed: {err}"),
    )
}

#[derive(Clone)]
struct FetchCache {
    root: Arc<Dir>,
    relative: Utf8PathBuf,
}

impl FetchCache {
    fn new(config: &NetworkConfig) -> Self {
        Self {
            root: Arc::clone(&config.cache_root),
            relative: config.cache_relative.clone(),
        }
    }

    fn open_dir(&self) -> Result<Dir, Error> {
        open_cache_dir(&self.root, &self.relative)
    }
}

#[derive(Clone)]
struct FetchContext {
    cache: FetchCache,
    policy: Arc<NetworkPolicy>,
}

impl FetchContext {
    fn new(config: NetworkConfig) -> Self {
        Self {
            cache: FetchCache::new(&config),
            policy: Arc::new(config.policy),
        }
    }

    fn open_cache_dir(&self) -> Result<Dir, Error> {
        self.cache.open_dir()
    }

    fn policy(&self) -> &NetworkPolicy {
        self.policy.as_ref()
    }
}

#[cfg(test)]
mod tests {
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

    fn assert_open_cache_dir_rejects(root: &Dir, path: &Utf8Path, description: &str) -> Result<()> {
        let err =
            open_cache_dir(root, path).expect_err("open_cache_dir should reject invalid path");
        ensure!(
            err.to_string().contains(description),
            "error should mention {description}, got {err}",
        );
        Ok(())
    }

    type CacheWorkspace = (tempfile::TempDir, Arc<Dir>, Utf8PathBuf);
}
