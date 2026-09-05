//! Network helpers exposed to `MiniJinja` templates.
//!
//! This module wires the high-level `fetch` helper that retrieves remote
//! resources with optional on-disk caching. Request validation lives in the
//! sibling [`policy`] module so the I/O-focused code here stays small and
//! explicit.

mod cache;
mod policy;
mod redirect;
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
};

#[cfg(test)]
use self::cache::open_cache_dir;
use self::cache::{CacheEntry, FetchCache, cache_key, discard_partial_cache, read_cached};
use self::redirect::dispatch_request;
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

/// Fetch a URL for the `fetch` template function, applying policy and optional caching.
///
/// # Errors
///
/// Returns an error when the template arguments are invalid or unused, the URL
/// cannot be parsed, or the network policy rejects it. When caching is
/// enabled, cache directory, entry, read, write, and sync failures are also
/// reported. Remote request failures, response-body read failures, and
/// responses that exceed the configured size limit are reported as errors.
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

    match context.policy().evaluate(&parsed) {
        Ok(()) => {
            tracing::debug!(
                operation = "fetch",
                policy_outcome = "allowed",
                "network policy allowed fetch"
            );
        }
        Err(violation) => {
            tracing::debug!(
                operation = "fetch",
                policy_outcome = "rejected",
                policy_reason = network_policy_rejection_reason(&violation),
                "network policy rejected fetch"
            );
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                localization::message(keys::STDLIB_FETCH_DISALLOWED)
                    .with_arg("url", url)
                    .with_arg("details", violation.to_string())
                    .to_string(),
            ));
        }
    }

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
            fetch_remote_with_cache(&parsed, context, impure, &cache)?
        }
    } else {
        fetch_remote(&parsed, context, impure)?
    };

    Ok(value_from_bytes(bytes))
}

/// Return a stable category for a rejected network-policy evaluation.
const fn network_policy_rejection_reason(violation: &NetworkPolicyViolation) -> &'static str {
    match violation {
        NetworkPolicyViolation::SchemeNotAllowed { .. } => "scheme_not_allowed",
        NetworkPolicyViolation::MissingHost { .. } => "missing_host",
        NetworkPolicyViolation::HostNotAllowlisted { .. } => "host_not_allowlisted",
        NetworkPolicyViolation::HostBlocked { .. } => "host_blocked",
    }
}

/// Fetch a URL's response body, enforcing the response size limit.
///
/// # Errors
///
/// Returns an error when the request cannot be dispatched, the response body
/// cannot be read, its buffer cannot be sliced, or the body exceeds `limit`
/// bytes.
fn fetch_remote(
    url: &Url,
    context: &FetchContext,
    impure: &Arc<AtomicBool>,
) -> Result<Vec<u8>, Error> {
    let response = dispatch_request(url, context.policy(), impure)?;
    read_response(
        url,
        response.into_reader(),
        context.max_response_bytes(),
        None,
    )
}

/// Fetch a URL, streaming the response into the cache entry.
///
/// # Errors
///
/// Returns an error when the request cannot be dispatched, the cache entry
/// cannot be opened or synchronized, the response cannot be read or copied to
/// the cache, or the response exceeds `limit` bytes. A partial cache entry is
/// removed when response processing fails.
fn fetch_remote_with_cache(
    url: &Url,
    context: &FetchContext,
    impure: &Arc<AtomicBool>,
    cache: &CacheEntry<'_>,
) -> Result<Vec<u8>, Error> {
    let response = dispatch_request(url, context.policy(), impure)?;
    let limit = context.max_response_bytes();
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

/// Read a response body up to the size limit, mirroring bytes to an optional cache sink.
///
/// # Errors
///
/// Returns an error when reading a response chunk fails, the body exceeds
/// `limit`, the response buffer cannot be sliced as requested, or an optional
/// cache sink rejects a write.
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

/// Read one chunk from the response body, localizing read failures.
///
/// # Errors
///
/// Returns an error when `reader` fails while reading the next response chunk.
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

/// Build the error for a live response that exceeds the size limit.
fn response_limit_error(url: &Url, limit: u64) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        localization::message(keys::STDLIB_FETCH_RESPONSE_LIMIT_EXCEEDED)
            .with_arg("url", url.as_str())
            .with_arg("limit", limit)
            .to_string(),
    )
}

/// Build the error for a cached entry that exceeds the size limit.
fn response_limit_error_from_cache(name: &str, limit: u64) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        localization::message(keys::STDLIB_FETCH_CACHE_LIMIT_EXCEEDED)
            .with_arg("name", name)
            .with_arg("limit", limit)
            .to_string(),
    )
}

/// Translate an I/O failure into a localized template error.
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
    /// The on-disk cache used when `cache=true` is requested.
    cache: FetchCache,
    /// The policy every fetch is validated against.
    policy: Arc<NetworkPolicy>,
    /// The maximum response body size in bytes.
    max_response_bytes: u64,
}

impl FetchContext {
    /// Build a context from the network configuration.
    fn new(config: NetworkConfig) -> Self {
        Self {
            cache: FetchCache::new(&config),
            policy: Arc::new(config.policy),
            max_response_bytes: config.max_response_bytes,
        }
    }

    /// Open the configured cache directory.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured relative path violates the cache
    /// boundary rules, or when the directory cannot be created or opened.
    #[rustfmt::skip]
    fn open_cache_dir(&self) -> Result<Dir, Error> { self.cache.open_dir() }

    /// Return the context's network policy.
    #[rustfmt::skip]
    fn policy(&self) -> &NetworkPolicy { self.policy.as_ref() }

    /// Return the context's response size limit.
    #[rustfmt::skip]
    const fn max_response_bytes(&self) -> u64 { self.max_response_bytes }
}

#[cfg(test)]
mod observability_tests;
#[cfg(test)]
mod redirect_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
#[path = "tests_support.rs"]
mod tests_support;
