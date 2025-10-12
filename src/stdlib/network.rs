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

use super::value_from_bytes;
use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{
    Environment, Error, ErrorKind,
    value::{Kwargs, Value},
};
use sha2::{Digest, Sha256};

const DEFAULT_CACHE_DIR: &str = ".netsuke/fetch";

pub(crate) fn register_functions(env: &mut Environment<'_>, impure: Arc<AtomicBool>) {
    env.add_function("fetch", move |url: String, kwargs: Kwargs| {
        fetch(&url, &kwargs, &impure)
    });
}

fn fetch(url: &str, kwargs: &Kwargs, impure: &Arc<AtomicBool>) -> Result<Value, Error> {
    let use_cache = kwargs.get::<Option<bool>>("cache")?.unwrap_or(false);
    let cache_dir = kwargs.get::<Option<String>>("cache_dir")?;
    kwargs.assert_all_used()?;

    let bytes = if use_cache {
        let dir = open_cache_dir(cache_dir.as_deref().unwrap_or(DEFAULT_CACHE_DIR))?;
        let key = cache_key(url);
        if let Some(cached) = read_cached(&dir, &key)? {
            impure.store(true, Ordering::Relaxed);
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

fn open_cache_dir(path: &str) -> Result<Dir, Error> {
    let utf_path = Utf8Path::new(path);
    if utf_path.as_str().is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "cache_dir must not be empty",
        ));
    }

    if utf_path.is_absolute() {
        let mut root = String::new();
        for component in utf_path.components() {
            match component {
                Utf8Component::Prefix(prefix) => root.push_str(prefix.as_str()),
                Utf8Component::RootDir => {
                    root.push('/');
                    break;
                }
                _ => break,
            }
        }
        if root.is_empty() {
            root.push('/');
        }
        let root_path = Utf8PathBuf::from(root);

        let root_dir = Dir::open_ambient_dir(&root_path, ambient_authority())
            .map_err(|err| io_error("open root cache dir", &root_path, &err))?;
        let rel = utf_path.strip_prefix(&root_path).map_err(|_| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!("failed to compute relative path for '{utf_path}'"),
            )
        })?;
        if rel.as_str().is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                "cache_dir must not be the filesystem root",
            ));
        }

        root_dir
            .create_dir_all(rel)
            .map_err(|err| io_error("create cache dir", utf_path, &err))?;
        return root_dir
            .open_dir(rel)
            .map_err(|err| io_error("open cache dir", utf_path, &err));
    }

    let cwd = Dir::open_ambient_dir(".", ambient_authority())
        .map_err(|err| io_error("open working dir", utf_path, &err))?;
    cwd.create_dir_all(utf_path)
        .map_err(|err| io_error("create cache dir", utf_path, &err))?;
    cwd.open_dir(utf_path)
        .map_err(|err| io_error("open cache dir", utf_path, &err))
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
        let _ = write!(out, "{byte:02x}");
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
    use super::*;

    use std::{
        fs,
        path::{Path, PathBuf},
        sync::{Mutex, MutexGuard, OnceLock},
    };

    use tempfile::tempdir;

    fn cwd_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct DirGuard {
        original: PathBuf,
        _lock: MutexGuard<'static, ()>,
    }

    impl DirGuard {
        fn change_to(path: &Path) -> Self {
            let lock = cwd_lock().lock().expect("cwd lock");
            let original = std::env::current_dir().expect("current dir");
            std::env::set_current_dir(path).expect("set current dir");
            Self {
                original,
                _lock: lock,
            }
        }
    }

    impl Drop for DirGuard {
        fn drop(&mut self) {
            std::env::set_current_dir(&self.original).expect("restore current dir");
        }
    }

    #[test]
    fn cache_key_stable() {
        assert_eq!(
            cache_key("http://example.com"),
            cache_key("http://example.com")
        );
    }

    #[test]
    fn hex_string_formats_bytes() {
        assert_eq!(hex_string(&[0x0f, 0xa0, 0x3c]), "0fa03c");
    }

    #[test]
    fn to_value_preserves_utf8() {
        let value = value_from_bytes(b"payload".to_vec());
        assert_eq!(value.as_str(), Some("payload"));
    }

    #[test]
    fn to_value_returns_bytes_for_invalid_utf8() {
        let value = value_from_bytes(vec![0xff, 0xfe, 0xfd]);
        assert_eq!(value.as_bytes(), Some(&[0xff, 0xfe, 0xfd][..]));
    }

    #[test]
    fn open_cache_dir_rejects_empty_path() {
        let err = open_cache_dir("").expect_err("empty path should fail");
        assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    }

    #[test]
    fn open_cache_dir_errors_for_file_path() {
        let temp = tempdir().expect("tempdir");
        let file_path = temp.path().join("file");
        fs::write(&file_path, b"data").expect("write file");
        let utf_path = Utf8PathBuf::from_path_buf(file_path).expect("utf8 path");
        let err = open_cache_dir(utf_path.as_str()).expect_err("file path should fail");
        assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    }

    #[test]
    fn open_cache_dir_creates_relative_directory() {
        let temp = tempdir().expect("tempdir");
        let _guard = DirGuard::change_to(temp.path());
        let dir = open_cache_dir("cache").expect("open relative cache dir");
        dir.write("entry", b"data").expect("write cache entry");
        drop(dir);
        let entry = temp.path().join("cache").join("entry");
        assert!(fs::metadata(entry).is_ok(), "cache entry should exist");
    }

    #[test]
    fn open_cache_dir_creates_absolute_directory() {
        let temp = tempdir().expect("tempdir");
        let absolute =
            Utf8PathBuf::from_path_buf(temp.path().join("cache")).expect("utf8 cache path");
        let dir = open_cache_dir(absolute.as_str()).expect("open absolute cache dir");
        dir.write("entry", b"data").expect("write cache entry");
        let entry = absolute.join("entry");
        assert!(
            fs::metadata(entry.as_std_path()).is_ok(),
            "cache entry should exist"
        );
    }
}
