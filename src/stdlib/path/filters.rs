//! Registration of stdlib path and file filters for `MiniJinja`.
//!
//! Exposes filters such as `basename`, `dirname`, `with_suffix`,
//! `relative_to`, `realpath`, `expanduser`, `size`, `contents`,
//! `linecount`, `hash`, and `digest`.
use camino::Utf8Path;
use minijinja::{Environment, Error, ErrorKind, value::Kwargs};

use super::{fs_utils, hash_utils, path_utils};
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
            let chosen_encoding = encoding.unwrap_or_else(|| "utf-8".to_owned());
            match chosen_encoding.to_ascii_lowercase().as_str() {
                "utf-8" | "utf8" => {
                    let limits = path_call_limits(&kwargs, file_max_read_bytes)?;
                    kwargs.assert_all_used()?;
                    fs_utils::read_utf8(Utf8Path::new(&raw), &limits)
                }
                other => Err(Error::new(
                    ErrorKind::InvalidOperation,
                    localization::message(keys::STDLIB_PATH_UNSUPPORTED_ENCODING)
                        .with_arg("encoding", other)
                        .to_string(),
                )),
            }
        },
    );
    env.add_filter(
        "linecount",
        move |raw: String, kwargs: Kwargs| -> Result<usize, Error> {
            let limits = path_call_limits(&kwargs, file_max_read_bytes)?;
            kwargs.assert_all_used()?;
            fs_utils::linecount(Utf8Path::new(&raw), &limits)
        },
    );
    env.add_filter(
        "hash",
        move |raw: String, alg: Option<String>, kwargs: Kwargs| -> Result<String, Error> {
            let algorithm = alg.unwrap_or_else(|| "sha256".to_owned());
            let limits = path_call_limits(&kwargs, file_max_read_bytes)?;
            kwargs.assert_all_used()?;
            hash_utils::compute_hash(Utf8Path::new(&raw), &algorithm, &limits)
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
            let limits = path_call_limits(&kwargs, file_max_read_bytes)?;
            kwargs.assert_all_used()?;
            hash_utils::compute_digest(Utf8Path::new(&raw), digest_len, &algorithm, &limits)
        },
    );
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
