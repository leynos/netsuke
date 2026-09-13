//! Registration of stdlib path and file filters for `MiniJinja`.
//!
//! Exposes filters such as `basename`, `dirname`, `with_suffix`,
//! `relative_to`, `realpath`, `expanduser`, `size`, `contents`,
//! `linecount`, `hash`, and `digest`.
use camino::Utf8Path;
use minijinja::{Environment, Error, ErrorKind, value::Kwargs};

use super::{bounded_read, fs_utils, hash_utils, path_utils, read_telemetry};
use crate::localization::{self, keys};
use crate::stdlib::config_types::HomeDirectory;
use crate::stdlib::path::fs_utils::FileReadLimits;

/// Register the `expanduser` filter.
///
/// Composition root: the one sanctioned ambient environment read. The
/// process-backed reader is captured here, at the registration boundary, and
/// injected into `expanduser`, so `path_utils` holds no process access and
/// `HomeDirectory::Ambient` consults whatever reader registration supplies.
fn register_expanduser(env: &mut Environment<'_>, home_directory: HomeDirectory) {
    #[expect(
        clippy::disallowed_methods,
        reason = "composition root: registration captures the process-backed reader once and injects it into the home ladders"
    )]
    let read_env = |key: &str| std::env::var(key).ok();
    env.add_filter("expanduser", move |raw: String| -> Result<String, Error> {
        path_utils::expanduser(&raw, &home_directory, read_env)
    });
}

/// Register path filters that transform strings without inspecting the host.
///
/// This deliberately limited surface is shared by manifest discovery queries.
/// Add a filter here only when it is entirely lexical; filters that inspect the
/// filesystem or environment belong exclusively in [`register_filters`].
fn register_lexical_filters(env: &mut Environment<'_>) {
    env.add_filter("basename", |raw: String| -> Result<String, Error> {
        Ok(path_utils::basename(Utf8Path::new(&raw)))
    });
    env.add_filter("dirname", |raw: String| -> Result<String, Error> {
        Ok(path_utils::dirname(Utf8Path::new(&raw)))
    });
    env.add_filter(
        "with_suffix",
        |raw: String,
         suffix: String,
         count: Option<usize>,
         sep: Option<String>|
         -> Result<String, Error> {
            let count_value = count.unwrap_or(1);
            let sep_value = sep.unwrap_or_else(|| ".".to_owned());
            path_utils::with_suffix(Utf8Path::new(&raw), &suffix, count_value, &sep_value)
                .map(camino::Utf8PathBuf::into_string)
        },
    );
    env.add_filter(
        "relative_to",
        |raw: String, root: String| -> Result<String, Error> {
            path_utils::relative_to(Utf8Path::new(&raw), Utf8Path::new(&root))
        },
    );
}

/// Register path filters safe for manifest discovery queries.
pub(crate) fn register_query_filters(env: &mut Environment<'_>) {
    register_lexical_filters(env);
}

/// Register the file-inspecting path filters and the `expanduser` filter.
pub(crate) fn register_filters(
    env: &mut Environment<'_>,
    home_directory: HomeDirectory,
    file_max_read_bytes: u64,
) {
    register_lexical_filters(env);
    env.add_filter("realpath", |raw: String| -> Result<String, Error> {
        path_utils::canonicalize_any(Utf8Path::new(&raw)).map(camino::Utf8PathBuf::into_string)
    });
    register_expanduser(env, home_directory);
    env.add_filter("size", |raw: String| -> Result<u64, Error> {
        fs_utils::file_size(Utf8Path::new(&raw))
    });
    // Templates using `contents` read from the ambient file system; enable the stdlib only for trusted templates.
    env.add_filter(
        "contents",
        move |raw: String, encoding: Option<String>, kwargs: Kwargs| -> Result<String, Error> {
            read_contents(&raw, encoding.as_deref(), &kwargs, file_max_read_bytes)
        },
    );
    env.add_filter(
        "linecount",
        move |raw: String, kwargs: Kwargs| -> Result<usize, Error> {
            read_bounded(
                read_telemetry::FILTER_LINECOUNT,
                &kwargs,
                file_max_read_bytes,
                |limits| bounded_read::linecount(Utf8Path::new(&raw), limits),
            )
        },
    );
    env.add_filter(
        "hash",
        move |raw: String, alg: Option<String>, kwargs: Kwargs| -> Result<String, Error> {
            let algorithm = alg.unwrap_or_else(|| "sha256".to_owned());
            read_bounded(
                read_telemetry::FILTER_HASH,
                &kwargs,
                file_max_read_bytes,
                |limits| hash_utils::compute_hash(Utf8Path::new(&raw), &algorithm, limits),
            )
        },
    );
    env.add_filter(
        "digest",
        move |raw: String,
              len: Option<usize>,
              alg: Option<String>,
              kwargs: Kwargs|
              -> Result<String, Error> {
            let digest_len = len.unwrap_or(8);
            let algorithm = alg.unwrap_or_else(|| "sha256".to_owned());
            read_bounded(
                read_telemetry::FILTER_DIGEST,
                &kwargs,
                file_max_read_bytes,
                |limits| {
                    hash_utils::compute_digest(Utf8Path::new(&raw), digest_len, &algorithm, limits)
                },
            )
        },
    );
}

/// Read `raw` as text in the requested `encoding`.
///
/// `contents` is the one reading filter whose encoding is selectable, so the
/// dispatch lives here rather than inline in the registration body: only the
/// UTF-8 arm reads a file, and any other encoding is refused before the
/// filesystem is touched.
///
/// The keywords are resolved, and any undeclared one refused, before the
/// encoding is judged. A call such as `contents(encoding="utf-16",
/// max_byte=1)` is wrong about its keyword, and that mistake must be reported
/// as such rather than hidden behind the encoding the same call also asked
/// for. Every refusal here is counted, the encoding included, so the recorded
/// series tally calls rather than reads alone.
///
/// # Errors
///
/// Returns a template error when a keyword is undeclared or malformed, when
/// the encoding is unsupported, or whatever [`read_bounded`] returns for the
/// requested file.
fn read_contents(
    raw: &str,
    encoding: Option<&str>,
    kwargs: &Kwargs,
    configured_max_read_bytes: u64,
) -> Result<String, Error> {
    let filter = read_telemetry::FILTER_CONTENTS;
    let limits = match path_call_limits(kwargs, configured_max_read_bytes) {
        Ok(limits) => limits,
        Err(err) => return read_telemetry::record_unresolved_read(filter, err),
    };
    if let Err(err) = kwargs.assert_all_used() {
        return read_telemetry::record_unresolved_read(filter, err);
    }
    let chosen_encoding = encoding.unwrap_or("utf-8").to_ascii_lowercase();
    match chosen_encoding.as_str() {
        "utf-8" | "utf8" => read_and_record(filter, &limits, |resolved| {
            bounded_read::read_utf8(Utf8Path::new(raw), resolved)
        }),
        other => {
            let unsupported = Error::new(
                ErrorKind::InvalidOperation,
                localization::message(keys::STDLIB_PATH_UNSUPPORTED_ENCODING)
                    .with_arg("encoding", other)
                    .to_string(),
            );
            read_telemetry::record_file_read(filter, Some(&limits), Err(unsupported))
        }
    }
}

/// Run one bounded read for `filter`, resolving its keywords on the way.
///
/// The three filters without a selectable encoding share this prologue:
/// resolve the per-call limits from the kwargs, refuse a keyword the filter
/// does not declare, then hand the read to [`read_and_record`]. Keeping it in
/// one place means a filter cannot quietly skip a step, and `read` receives
/// the same limits that the recorded call ran under rather than a second,
/// possibly stale, resolution. A call refused by either check is recorded too:
/// it reads nothing, but it is still a filter outcome.
fn read_bounded<T>(
    filter: &'static str,
    kwargs: &Kwargs,
    configured_max_read_bytes: u64,
    read: impl FnOnce(&FileReadLimits) -> Result<T, Error>,
) -> Result<T, Error> {
    let limits = match path_call_limits(kwargs, configured_max_read_bytes) {
        Ok(limits) => limits,
        Err(err) => return read_telemetry::record_unresolved_read(filter, err),
    };
    if let Err(err) = kwargs.assert_all_used() {
        return read_telemetry::record_unresolved_read(filter, err);
    }
    read_and_record(filter, &limits, read)
}

/// Run one bounded read under `limits` and record its outcome once.
///
/// [`read_contents`] resolves its keywords before judging the encoding, so it
/// arrives here with the limits already in hand; routing both entry points
/// through this call keeps exactly one recording per filter result.
fn read_and_record<T>(
    filter: &'static str,
    limits: &FileReadLimits,
    read: impl FnOnce(&FileReadLimits) -> Result<T, Error>,
) -> Result<T, Error> {
    read_telemetry::record_file_read(filter, Some(limits), read(limits))
}

/// Resolve the per-call read limits from `max_bytes` and `follow_symlinks` kwargs.
///
/// `max_bytes` may only narrow the operator-configured ceiling: a call that
/// asks for more bytes than the configured budget is clamped to the budget
/// rather than granted a larger read.
fn path_call_limits(
    kwargs: &Kwargs,
    configured_max_read_bytes: u64,
) -> Result<FileReadLimits, Error> {
    let max_bytes: Option<u64> = kwargs.get("max_bytes")?;
    let follow_symlinks: Option<bool> = kwargs.get("follow_symlinks")?;
    Ok(FileReadLimits {
        max_bytes: match max_bytes {
            Some(requested) if requested < configured_max_read_bytes => requested,
            _ => configured_max_read_bytes,
        },
        follow_symlinks: follow_symlinks.unwrap_or(false),
    })
}
