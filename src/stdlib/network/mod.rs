//! Network helpers exposed to `MiniJinja` templates.
//!
//! This module wires the high-level `fetch` helper that retrieves remote
//! resources with optional on-disk caching. Request validation lives in the
//! sibling [`policy`] module so the I/O-focused code here stays small and
//! explicit.

mod cache;
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

use self::cache::{CacheEntry, FetchCache, cache_key, discard_partial_cache, read_cached};
#[cfg(test)]
use self::cache::{hex_string, open_cache_dir};
use super::{NetworkConfig, StdlibConfig, value_from_bytes};
use crate::localization::{self, keys};
use crate::stdlib::io_helpers::io_action_error;
use camino::Utf8Path;
use cap_std::fs_utf8::Dir;
use minijinja::{
    Environment, Error, ErrorKind,
    value::{Kwargs, Value},
};
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
            localization::message(keys::STDLIB_FETCH_URL_INVALID)
                .with_arg("url", url)
                .with_arg("details", err.to_string())
                .to_string(),
        )
    })?;

    context.policy().evaluate(&parsed).map_err(|violation| {
        Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::STDLIB_FETCH_DISALLOWED)
                .with_arg("url", url)
                .with_arg("details", violation.to_string())
                .to_string(),
        )
    })?;

    let limit = context.max_response_bytes();
    let bytes = if use_cache {
        let dir = context.open_cache_dir()?;
        let key = cache_key(parsed.as_str());
        // Log the host and cache key rather than the full URL: the parsed URL
        // may carry userinfo, and the key already identifies the entry uniquely.
        let host = parsed.host_str().unwrap_or("");
        if let Some(cached) = read_cached(&dir, &key, limit)? {
            tracing::debug!(host, key = %key, "fetch cache hit");
            impure.store(true, Ordering::Relaxed);
            cached
        } else {
            tracing::debug!(host, key = %key, "fetch cache miss");
            let cache = CacheEntry::new(&dir, &key);
            fetch_remote_with_cache(&parsed, impure, limit, &cache)?
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
    limit: u64,
    cache: &CacheEntry<'_>,
) -> Result<Vec<u8>, Error> {
    let response = dispatch_request(url, impure)?;
    let mut file = cache.open_writer()?;
    match read_response(url, response.into_reader(), limit, Some(&mut file)) {
        Ok(bytes) => {
            file.sync_all()
                .map_err(|err| io_error(keys::STDLIB_FETCH_ACTION_SYNC_CACHE, cache.path(), err))?;
            Ok(bytes)
        }
        Err(err) => {
            drop(file);
            discard_partial_cache(cache);
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
        // Log the host, not the full URL, which may carry userinfo.
        tracing::warn!(host = url.host_str().unwrap_or(""), error = %err, "fetch request failed");
        Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::STDLIB_FETCH_FAILED)
                .with_arg("url", url.as_str())
                .with_arg("details", err.to_string())
                .to_string(),
        )
    })
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
        let read = read_response_chunk(url, &mut reader, &mut chunk)?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        if total > limit {
            return Err(response_limit_error(url, limit));
        }
        let bytes = chunk.get(..read).ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidOperation,
                localization::message(keys::STDLIB_FETCH_RESPONSE_BUFFER_OVERFLOW)
                    .with_arg("url", url.as_str())
                    .to_string(),
            )
        })?;
        buffer.extend_from_slice(bytes);
        copy_to_sink(url, bytes, &mut sink)?;
    }
    Ok(buffer)
}

/// Read one chunk from the response body, localising read failures.
fn read_response_chunk(
    url: &Url,
    reader: &mut impl Read,
    chunk: &mut [u8],
) -> Result<usize, Error> {
    reader.read(chunk).map_err(|err| {
        Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::STDLIB_FETCH_RESPONSE_READ_FAILED)
                .with_arg("url", url.as_str())
                .with_arg("details", err.to_string())
                .to_string(),
        )
    })
}

/// Mirror a response chunk into the optional cache writer.
fn copy_to_sink(url: &Url, bytes: &[u8], sink: &mut Option<&mut dyn Write>) -> Result<(), Error> {
    let Some(writer) = sink.as_deref_mut() else {
        return Ok(());
    };
    writer.write_all(bytes).map_err(|err| {
        Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::STDLIB_FETCH_CACHE_WRITE_FAILED)
                .with_arg("url", url.as_str())
                .with_arg("details", err.to_string())
                .to_string(),
        )
    })
}

fn response_limit_error(url: &Url, limit: u64) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        localization::message(keys::STDLIB_FETCH_RESPONSE_LIMIT_EXCEEDED)
            .with_arg("url", url.as_str())
            .with_arg("limit", limit)
            .to_string(),
    )
}

fn response_limit_error_from_cache(name: &str, limit: u64) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        localization::message(keys::STDLIB_FETCH_CACHE_LIMIT_EXCEEDED)
            .with_arg("name", name)
            .with_arg("limit", limit)
            .to_string(),
    )
}

fn io_error(action_key: &'static str, path: &Utf8Path, err: io::Error) -> Error {
    io_action_error(
        keys::STDLIB_FETCH_IO_FAILED,
        &localization::message(action_key),
        path,
        err,
    )
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
    const fn max_response_bytes(&self) -> u64 { self.max_response_bytes }
}

#[cfg(test)]
mod tests;
