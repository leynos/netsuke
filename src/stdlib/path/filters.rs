use camino::Utf8Path;
use minijinja::{Environment, Error, ErrorKind};

use super::{fs_utils, hash_utils, path_utils};

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
            let count = count.unwrap_or(1);
            let sep = sep.unwrap_or_else(|| ".".to_string());
            path_utils::with_suffix(Utf8Path::new(&raw), &suffix, count, &sep)
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
    env.add_filter(
        "contents",
        |raw: String, encoding: Option<String>| -> Result<String, Error> {
            let encoding = encoding.unwrap_or_else(|| "utf-8".to_string());
            match encoding.to_ascii_lowercase().as_str() {
                "utf-8" | "utf8" => fs_utils::read_utf8(Utf8Path::new(&raw)),
                other => Err(Error::new(
                    ErrorKind::InvalidOperation,
                    format!("unsupported encoding '{other}'"),
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
            let alg = alg.unwrap_or_else(|| "sha256".to_string());
            hash_utils::compute_hash(Utf8Path::new(&raw), &alg)
        },
    );
    env.add_filter(
        "digest",
        |raw: String, len: Option<usize>, alg: Option<String>| -> Result<String, Error> {
            let len = len.unwrap_or(8);
            let alg = alg.unwrap_or_else(|| "sha256".to_string());
            hash_utils::compute_digest(Utf8Path::new(&raw), len, &alg)
        },
    );
}
