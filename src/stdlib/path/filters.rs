//! Registration of stdlib path and file filters for `MiniJinja`.
//!
//! Exposes filters such as `basename`, `dirname`, `with_suffix`,
//! `relative_to`, `realpath`, `expanduser`, `size`, `contents`,
//! `linecount`, `hash`, and `digest`.
use camino::Utf8Path;
use minijinja::{Environment, Error, ErrorKind};

use super::{fs_utils, hash_utils, path_utils};
use crate::localization::{self, keys};

pub(crate) fn register_filters(env: &mut Environment<'_>) {
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
    env.add_filter("realpath", |raw: String| -> Result<String, Error> {
        path_utils::canonicalize_any(Utf8Path::new(&raw)).map(camino::Utf8PathBuf::into_string)
    });
    env.add_filter("expanduser", |raw: String| -> Result<String, Error> {
        path_utils::expanduser(&raw)
    });
    env.add_filter("size", |raw: String| -> Result<u64, Error> {
        fs_utils::file_size(Utf8Path::new(&raw))
    });
    // Templates using `contents` read from the ambient file system; enable the stdlib only for trusted templates.
    env.add_filter(
        "contents",
        |raw: String, encoding: Option<String>| -> Result<String, Error> {
            let chosen_encoding = encoding.unwrap_or_else(|| "utf-8".to_owned());
            match chosen_encoding.to_ascii_lowercase().as_str() {
                "utf-8" | "utf8" => fs_utils::read_utf8(Utf8Path::new(&raw)),
                other => Err(Error::new(
                    ErrorKind::InvalidOperation,
                    localization::message(keys::STDLIB_PATH_UNSUPPORTED_ENCODING)
                        .with_arg("encoding", other)
                        .to_string(),
                )),
            }
        },
    );
    env.add_filter("linecount", |raw: String| -> Result<usize, Error> {
        fs_utils::linecount(Utf8Path::new(&raw))
    });
    env.add_filter(
        "hash",
        |raw: String, alg: Option<String>| -> Result<String, Error> {
            let algorithm = alg.unwrap_or_else(|| "sha256".to_owned());
            hash_utils::compute_hash(Utf8Path::new(&raw), &algorithm)
        },
    );
    env.add_filter(
        "digest",
        |raw: String, len: Option<usize>, alg: Option<String>| -> Result<String, Error> {
            let digest_len = len.unwrap_or(8);
            let algorithm = alg.unwrap_or_else(|| "sha256".to_owned());
            hash_utils::compute_digest(Utf8Path::new(&raw), digest_len, &algorithm)
        },
    );
}
