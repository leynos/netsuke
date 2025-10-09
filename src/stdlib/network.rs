//! Network helpers exposed to `MiniJinja` templates.
//!
//! Currently this module provides the `fetch` function that retrieves remote
//! resources with optional on-disk caching.

use std::io::{self, Read};

use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{
    Environment, Error, ErrorKind,
    value::{Kwargs, Value},
};
use sha2::{Digest, Sha256};

const DEFAULT_CACHE_DIR: &str = ".netsuke/fetch";

pub(crate) fn register_functions(env: &mut Environment<'_>) {
    env.add_function("fetch", |url: String, kwargs: Kwargs| fetch(&url, &kwargs));
}

fn fetch(url: &str, kwargs: &Kwargs) -> Result<Value, Error> {
    let use_cache = kwargs.get::<Option<bool>>("cache")?.unwrap_or(false);
    let cache_dir = kwargs.get::<Option<String>>("cache_dir")?;
    kwargs.assert_all_used()?;

    let bytes = if use_cache {
        let dir = open_cache_dir(cache_dir.as_deref().unwrap_or(DEFAULT_CACHE_DIR))?;
        let key = cache_key(url);
        if let Some(cached) = read_cached(&dir, &key)? {
            cached
        } else {
            let data = fetch_remote(url)?;
            write_cache(&dir, &key, &data)?;
            data
        }
    } else {
        fetch_remote(url)?
    };

    Ok(to_value(bytes))
}

fn fetch_remote(url: &str) -> Result<Vec<u8>, Error> {
    let response = ureq::get(url).call().map_err(|err| {
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
        let root = Dir::open_ambient_dir("/", ambient_authority())
            .map_err(|err| io_error("open root cache dir", utf_path, &err))?;
        let rel = utf_path.strip_prefix("/").unwrap_or(utf_path);
        root.create_dir_all(rel)
            .map_err(|err| io_error("create cache dir", utf_path, &err))?;
        root.open_dir(rel)
            .map_err(|err| io_error("open cache dir", utf_path, &err))
    } else {
        let cwd = Dir::open_ambient_dir(".", ambient_authority())
            .map_err(|err| io_error("open working dir", utf_path, &err))?;
        cwd.create_dir_all(utf_path)
            .map_err(|err| io_error("create cache dir", utf_path, &err))?;
        cwd.open_dir(utf_path)
            .map_err(|err| io_error("open cache dir", utf_path, &err))
    }
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

fn write_cache(dir: &Dir, name: &str, data: &[u8]) -> Result<(), Error> {
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

fn to_value(bytes: Vec<u8>) -> Value {
    match String::from_utf8(bytes) {
        Ok(text) => Value::from(text),
        Err(err) => Value::from_bytes(err.into_bytes()),
    }
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

    #[test]
    fn cache_key_stable() {
        assert_eq!(
            cache_key("http://example.com"),
            cache_key("http://example.com")
        );
    }
}
