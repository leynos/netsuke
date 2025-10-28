//! Network helpers exposed to `MiniJinja` templates.
//!
//! This module wires the high-level `fetch` helper that retrieves remote
//! resources with optional on-disk caching. Request validation lives in the
//! sibling [`policy`] module so the I/O-focused code here stays small and
//! explicit.

mod policy;
pub use self::policy::{
    HostPatternError, NetworkPolicy, NetworkPolicyConfigError, NetworkPolicyViolation,
};

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
mod tests;
