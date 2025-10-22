//! Network helpers exposed to `MiniJinja` templates.
//!
//! Currently this module provides the `fetch` function that retrieves remote
//! resources with optional on-disk caching.

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

#[derive(Clone)]
struct FetchCache {
    root: Arc<Dir>,
    relative: Utf8PathBuf,
}

impl FetchCache {
    fn new(config: NetworkConfig) -> Self {
        Self {
            root: config.cache_root,
            relative: config.cache_relative,
        }
    }

    fn open_dir(&self) -> Result<Dir, Error> {
        open_cache_dir(&self.root, &self.relative)
    }
}

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
    let cache = Arc::new(FetchCache::new(config));
    env.add_function("fetch", move |url: String, kwargs: Kwargs| {
        fetch(&url, &kwargs, &impure, &cache)
    });
}

fn fetch(
    url: &str,
    kwargs: &Kwargs,
    impure: &Arc<AtomicBool>,
    cache: &FetchCache,
) -> Result<Value, Error> {
    let use_cache = kwargs.get::<Option<bool>>("cache")?.unwrap_or(false);
    kwargs.assert_all_used()?;

    let bytes = if use_cache {
        let dir = cache.open_dir()?;
        let key = cache_key(url);
        if let Some(cached) = read_cached(&dir, &key)? {
            cached
        } else {
            let data = fetch_remote(url, impure)?;
            write_cache(&dir, &key, &data, impure)?;
            data
        }
    } else {
        fetch_remote(url, impure)?
    };

    Ok(value_from_bytes(bytes))
}

fn fetch_remote(url: &str, impure: &Arc<AtomicBool>) -> Result<Vec<u8>, Error> {
    impure.store(true, Ordering::Relaxed);
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .timeout_write(Duration::from_secs(30))
        .timeout(Duration::from_secs(60))
        .build();
    let response = agent.get(url).call().map_err(|err| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("fetch failed for '{url}': {err}"),
        )
    })?;
    let mut reader = response.into_reader();
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).map_err(|err| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("failed to read response from '{url}': {err}"),
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
        if let Err(err) = write!(out, "{byte:02x}") {
            debug_assert!(false, "format hex byte failed: {err}");
        }
    }
    out
}

fn io_error(action: &str, path: &Utf8Path, err: &io::Error) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!("{action} for '{path}' failed: {err}"),
    )
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "tests cover IO-heavy scenarios succinctly"
    )]
    use super::*;

    use std::{
        fs,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    use crate::stdlib::DEFAULT_FETCH_CACHE_DIR;
    use camino::Utf8PathBuf;
    use cap_std::{ambient_authority, fs_utf8::Dir};
    use minijinja::value::{Kwargs, Value};
    use rstest::{fixture, rstest};
    use tempfile::tempdir;

    #[fixture]
    fn cache_workspace() -> (tempfile::TempDir, Arc<Dir>, Utf8PathBuf) {
        let temp = tempdir().expect("tempdir");
        let root_path = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8 path");
        let dir = Dir::open_ambient_dir(&root_path, ambient_authority()).expect("open workspace");
        (temp, Arc::new(dir), root_path)
    }

    fn make_cache(root: Arc<Dir>) -> FetchCache {
        FetchCache {
            root,
            relative: Utf8PathBuf::from(DEFAULT_FETCH_CACHE_DIR),
        }
    }

    #[rstest]
    fn cache_key_stable() {
        assert_eq!(
            cache_key("http://example.com"),
            cache_key("http://example.com"),
        );
    }

    #[rstest]
    fn hex_string_formats_bytes() {
        assert_eq!(hex_string(&[0x0f, 0xa0, 0x3c]), "0fa03c");
    }

    #[rstest]
    fn to_value_preserves_utf8() {
        let value = value_from_bytes(b"payload".to_vec());
        assert_eq!(value.as_str(), Some("payload"));
    }

    #[rstest]
    fn to_value_returns_bytes_for_invalid_utf8() {
        let value = value_from_bytes(vec![0xff, 0xfe, 0xfd]);
        assert_eq!(value.as_bytes(), Some(&[0xff, 0xfe, 0xfd][..]));
    }

    #[rstest]
    fn open_cache_dir_rejects_empty_path(
        cache_workspace: (tempfile::TempDir, Arc<Dir>, Utf8PathBuf),
    ) {
        let (_temp, root, _path) = cache_workspace;
        let err = open_cache_dir(&root, Utf8Path::new("")).expect_err("empty path should fail");
        assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    }

    #[rstest]
    fn open_cache_dir_rejects_absolute_paths(
        cache_workspace: (tempfile::TempDir, Arc<Dir>, Utf8PathBuf),
    ) {
        let (_temp, root, _path) = cache_workspace;
        let err = open_cache_dir(&root, Utf8Path::new("/etc/netsuke-cache"))
            .expect_err("absolute path should fail");
        assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    }

    #[rstest]
    fn open_cache_dir_rejects_parent_paths(
        cache_workspace: (tempfile::TempDir, Arc<Dir>, Utf8PathBuf),
    ) {
        let (_temp, root, _path) = cache_workspace;
        let err = open_cache_dir(&root, Utf8Path::new("../escape"))
            .expect_err("parent paths should fail");
        assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    }

    #[rstest]
    fn open_cache_dir_errors_for_file_path(
        cache_workspace: (tempfile::TempDir, Arc<Dir>, Utf8PathBuf),
    ) {
        let (_temp, root, path) = cache_workspace;
        let file_path = path.join("file");
        fs::write(file_path.as_std_path(), b"data").expect("write file");
        let err = open_cache_dir(&root, file_path.as_path()).expect_err("file path should fail");
        assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    }

    #[rstest]
    fn open_cache_dir_creates_relative_directory(
        cache_workspace: (tempfile::TempDir, Arc<Dir>, Utf8PathBuf),
    ) {
        let (_temp, root, path) = cache_workspace;
        let dir = open_cache_dir(&root, Utf8Path::new("cache")).expect("open relative cache dir");
        dir.write("entry", b"data").expect("write cache entry");
        drop(dir);
        let entry = path.join("cache").join("entry");
        assert!(
            fs::metadata(entry.as_std_path()).is_ok(),
            "cache entry should exist",
        );
    }

    #[rstest]
    fn fetch_rejects_template_cache_dir_argument(
        cache_workspace: (tempfile::TempDir, Arc<Dir>, Utf8PathBuf),
    ) {
        let (_temp, root, _path) = cache_workspace;
        let cache = make_cache(root);
        let kwargs =
            Kwargs::from_iter([(String::from("cache_dir"), Value::from(".netsuke/cache"))]);
        let impure = Arc::new(AtomicBool::new(false));
        let err = fetch("http://127.0.0.1:9", &kwargs, &impure, &cache)
            .expect_err("cache_dir keyword should be rejected");
        assert_eq!(err.kind(), ErrorKind::TooManyArguments);
        assert!(
            err.to_string().contains("cache_dir"),
            "error should mention unexpected cache_dir argument: {err}",
        );
        assert!(
            !impure.load(Ordering::Relaxed),
            "rejecting cache_dir must not mark the template impure",
        );
    }

    #[rstest]
    fn fetch_cache_opens_default_directory(
        cache_workspace: (tempfile::TempDir, Arc<Dir>, Utf8PathBuf),
    ) {
        let (_temp, root, path) = cache_workspace;
        let cache = make_cache(root);
        let dir = cache.open_dir().expect("open default cache dir");
        dir.write("entry", b"data").expect("write entry");
        drop(dir);
        let entry = path.join(DEFAULT_FETCH_CACHE_DIR).join("entry");
        assert!(
            fs::metadata(entry.as_std_path()).is_ok(),
            "entry should exist",
        );
    }
}
