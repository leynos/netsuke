//! Network helpers exposed to `MiniJinja` templates.
//!
//! This module wires the high-level `fetch` helper that retrieves remote
//! resources with optional on-disk caching. Request validation lives in the
//! sibling [`policy`] module so the I/O-focused code here stays small and
//! explicit.

mod policy;
/// Network policy that controls which schemes and hosts the fetch helper may reach.
pub use self::policy::NetworkPolicy;
/// Error returned when constructing an invalid network policy configuration.
pub use self::policy::NetworkPolicyConfigError;
/// Error returned when a URL violates the active network policy.
pub use self::policy::NetworkPolicyViolation;
/// Error returned when parsing an invalid host pattern.
pub use crate::host_pattern::HostPatternError;

use std::{
    convert::TryFrom,
    io::{self, Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use super::{NetworkConfig, StdlibConfig, value_from_bytes};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::{Dir, File, OpenOptions};
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

    let limit = context.max_response_bytes();
    let bytes = if use_cache {
        let dir = context.open_cache_dir()?;
        let key = cache_key(parsed.as_str());
        if let Some(cached) = read_cached(&dir, &key, limit)? {
            impure.store(true, Ordering::Relaxed);
            cached
        } else {
            fetch_remote_with_cache(&parsed, impure, &dir, &key, limit)?
        }
    } else {
        fetch_remote(&parsed, impure, limit)?
    };

    Ok(value_from_bytes(bytes))
}

fn fetch_remote(url: &Url, impure: &Arc<AtomicBool>, limit: u64) -> Result<Vec<u8>, Error> {
    let response = dispatch_request(url, impure)?;
    read_response(url, response.into_reader(), limit, None)
}

fn fetch_remote_with_cache(
    url: &Url,
    impure: &Arc<AtomicBool>,
    dir: &Dir,
    name: &str,
    limit: u64,
) -> Result<Vec<u8>, Error> {
    let response = dispatch_request(url, impure)?;
    let path = Utf8Path::new(name);
    let mut file = open_cache_writer(dir, path)?;
    let mut sink: Option<&mut dyn Write> = Some(&mut file);
    match read_response(url, response.into_reader(), limit, sink.take()) {
        Ok(bytes) => {
            file.sync_all()
                .map_err(|err| io_error("sync cache entry", path, &err))?;
            Ok(bytes)
        }
        Err(err) => {
            drop(file);
            if let Err(remove_err) = dir.remove_file(path) {
                if remove_err.kind() != io::ErrorKind::NotFound {
                    tracing::warn!("failed to clean up partial fetch cache '{name}': {remove_err}");
                }
            }
            Err(err)
        }
    }
}

fn dispatch_request(url: &Url, impure: &Arc<AtomicBool>) -> Result<ureq::Response, Error> {
    impure.store(true, Ordering::Relaxed);
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .timeout_write(Duration::from_secs(30))
        .timeout(Duration::from_secs(60))
        .build();
    agent.get(url.as_str()).call().map_err(|err| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("fetch failed for '{}': {err}", url.as_str()),
        )
    })
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

fn read_cached(dir: &Dir, name: &str, limit: u64) -> Result<Option<Vec<u8>>, Error> {
    let path = Utf8Path::new(name);
    let mut options = OpenOptions::new();
    options.read(true);
    match dir.open_with(path, &options) {
        Ok(mut file) => {
            let metadata = dir
                .metadata(path)
                .map_err(|err| io_error("stat cache entry", path, &err))?;
            if metadata.len() > limit {
                return Err(response_limit_error_from_cache(name, limit));
            }
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

fn read_response(
    url: &Url,
    mut reader: impl Read,
    limit: u64,
    mut sink: Option<&mut dyn Write>,
) -> Result<Vec<u8>, Error> {
    let mut total: u64 = 0;
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 8 * 1024];
    loop {
        let read = reader.read(&mut chunk).map_err(|err| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!("failed to read response from '{}': {err}", url.as_str()),
            )
        })?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        if total > limit {
            return Err(response_limit_error(url, limit));
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(writer) = sink.as_deref_mut() {
            writer.write_all(&chunk[..read]).map_err(|err| {
                Error::new(
                    ErrorKind::InvalidOperation,
                    format!("failed to write cache entry for '{}': {err}", url.as_str()),
                )
            })?;
        }
    }
    Ok(buffer)
}

fn open_cache_writer(dir: &Dir, path: &Utf8Path) -> Result<File, Error> {
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    dir.open_with(path, &options)
        .map_err(|err| io_error("open cache entry", path, &err))
}

fn response_limit_error(url: &Url, limit: u64) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!(
            "response from '{}' exceeded the configured limit of {} bytes",
            url.as_str(),
            limit
        ),
    )
}

fn response_limit_error_from_cache(name: &str, limit: u64) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!("cache entry '{name}' exceeded the configured fetch limit of {limit} bytes"),
    )
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

/// Internal cache configuration and directory handle.
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

    #[rustfmt::skip]
    fn open_dir(&self) -> Result<Dir, Error> { open_cache_dir(&self.root, &self.relative) }
}

/// Encapsulates fetch cache and network policy for template function registration.
#[derive(Clone)]
struct FetchContext {
    cache: FetchCache,
    policy: Arc<NetworkPolicy>,
    max_response_bytes: u64,
}

impl FetchContext {
    fn new(config: NetworkConfig) -> Self {
        Self {
            cache: FetchCache::new(&config),
            policy: Arc::new(config.policy),
            max_response_bytes: config.max_response_bytes,
        }
    }

    #[rustfmt::skip]
    fn open_cache_dir(&self) -> Result<Dir, Error> { self.cache.open_dir() }

    #[rustfmt::skip]
    fn policy(&self) -> &NetworkPolicy { self.policy.as_ref() }

    #[rustfmt::skip]
    fn max_response_bytes(&self) -> u64 { self.max_response_bytes }
}

#[cfg(test)]
mod tests;
