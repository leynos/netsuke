//! Standard library registration for `MiniJinja` templates.
//!
//! The module wires the platform-aware file tests, the path manipulation
//! filters, the collection helpers, the network utilities, and the command
//! wrappers into a single entrypoint so template authors can rely on
//! consistent behaviour across projects. Tests such as `dir`, `file`, and
//! `symlink` inspect metadata without following symlinks, while filters
//! expose conveniences like `basename`, `with_suffix`, `realpath`, content
//! hashing, collection utilities including `flatten`, `group_by`, and `uniq`,
//! HTTP helpers like `fetch`, and shell bridges such as `shell` and `grep`.

mod collections;
mod command;
mod network;
mod path;
mod time;

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
#[cfg(unix)]
use cap_std::fs::FileTypeExt;
use cap_std::{ambient_authority, fs, fs_utf8::Dir};
use minijinja::{Environment, Error, value::Value};
use std::{
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
};

type FileTest = (&'static str, fn(fs::FileType) -> bool);

pub(crate) const DEFAULT_FETCH_CACHE_DIR: &str = ".netsuke/fetch";

#[derive(Debug)]
pub struct StdlibConfig {
    workspace_root: Arc<Dir>,
    fetch_cache_relative: Utf8PathBuf,
}

impl StdlibConfig {
    #[must_use]
    pub fn new(workspace_root: Dir) -> Self {
        Self {
            workspace_root: Arc::new(workspace_root),
            fetch_cache_relative: Utf8PathBuf::from(DEFAULT_FETCH_CACHE_DIR),
        }
    }

    #[must_use]
    pub fn with_fetch_cache_relative(mut self, relative: impl Into<Utf8PathBuf>) -> Self {
        self.fetch_cache_relative = relative.into();
        self
    }

    pub(crate) fn network_config(self) -> NetworkConfig {
        NetworkConfig {
            cache_root: self.workspace_root,
            cache_relative: self.fetch_cache_relative,
        }
    }
}

impl Clone for StdlibConfig {
    fn clone(&self) -> Self {
        Self {
            workspace_root: Arc::clone(&self.workspace_root),
            fetch_cache_relative: self.fetch_cache_relative.clone(),
        }
    }
}

impl Default for StdlibConfig {
    fn default() -> Self {
        let root =
            Dir::open_ambient_dir(".", ambient_authority()).expect("open stdlib workspace root");
        Self::new(root)
    }
}

#[derive(Clone)]
pub(crate) struct NetworkConfig {
    pub(crate) cache_root: Arc<Dir>,
    pub(crate) cache_relative: Utf8PathBuf,
}

/// Captures mutable state shared between stdlib helpers.
#[derive(Clone, Default, Debug)]
pub struct StdlibState {
    impure: Arc<AtomicBool>,
}

impl StdlibState {
    /// Returns whether any impure helper executed during the last render.
    #[must_use]
    pub fn is_impure(&self) -> bool {
        self.impure.load(Ordering::Relaxed)
    }

    /// Resets the impurity marker so callers can track helper usage per render.
    pub fn reset_impure(&self) {
        self.impure.store(false, Ordering::Relaxed);
    }

    pub(crate) fn impure_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.impure)
    }
}

/// Register standard library helpers with the `MiniJinja` environment.
///
/// # Examples
/// ```
/// use minijinja::{context, Environment};
/// use netsuke::stdlib;
///
/// let mut env = Environment::new();
/// let _state = stdlib::register(&mut env).expect("register stdlib");
/// env.add_template("t", "{{ path | basename }}").expect("add template");
/// let tmpl = env.get_template("t").expect("get template");
/// let rendered = tmpl
///     .render(context!(path => "foo/bar.txt"))
///     .expect("render");
/// assert_eq!(rendered, "bar.txt");
/// ```
///
/// # Errors
///
/// Returns an error when the current working directory cannot be opened using
/// capability-based I/O. This occurs when the process lacks permission to read
/// the directory or if it no longer exists.
pub fn register(env: &mut Environment<'_>) -> Result<StdlibState> {
    let root = Dir::open_ambient_dir(".", ambient_authority())
        .context("open current directory for stdlib registration")?;
    Ok(register_with_config(env, StdlibConfig::new(root)))
}

pub fn register_with_config(env: &mut Environment<'_>, config: StdlibConfig) -> StdlibState {
    let state = StdlibState::default();
    register_file_tests(env);
    path::register_filters(env);
    collections::register_filters(env);
    let impure = state.impure_flag();
    network::register_functions(env, Arc::clone(&impure), config.network_config());
    command::register(env, impure);
    time::register_functions(env);
    state
}

pub(crate) fn value_from_bytes(bytes: Vec<u8>) -> Value {
    match String::from_utf8(bytes) {
        Ok(text) => Value::from(text),
        Err(err) => Value::from_bytes(err.into_bytes()),
    }
}

fn register_file_tests(env: &mut Environment<'_>) {
    const TESTS: &[FileTest] = &[
        ("dir", is_dir),
        ("file", is_file),
        ("symlink", is_symlink),
        ("pipe", is_fifo),
        ("block_device", is_block_device),
        ("char_device", is_char_device),
        ("device", is_device),
    ];

    for &(name, pred) in TESTS {
        env.add_test(name, move |val: Value| -> Result<bool, Error> {
            if let Some(s) = val.as_str() {
                return path::file_type_matches(Utf8Path::new(s), pred);
            }
            Ok(false)
        });
    }
}

fn is_dir(ft: fs::FileType) -> bool {
    ft.is_dir()
}

fn is_file(ft: fs::FileType) -> bool {
    ft.is_file()
}

fn is_symlink(ft: fs::FileType) -> bool {
    ft.is_symlink()
}

#[cfg(unix)]
fn is_fifo(ft: fs::FileType) -> bool {
    ft.is_fifo()
}

#[cfg(not(unix))]
fn is_fifo(_ft: fs::FileType) -> bool {
    false
}

#[cfg(unix)]
fn is_block_device(ft: fs::FileType) -> bool {
    ft.is_block_device()
}

#[cfg(not(unix))]
fn is_block_device(_ft: fs::FileType) -> bool {
    false
}

#[cfg(unix)]
fn is_char_device(ft: fs::FileType) -> bool {
    ft.is_char_device()
}

#[cfg(not(unix))]
fn is_char_device(_ft: fs::FileType) -> bool {
    false
}

#[cfg(unix)]
fn is_device(ft: fs::FileType) -> bool {
    is_block_device(ft) || is_char_device(ft)
}

#[cfg(not(unix))]
fn is_device(_ft: fs::FileType) -> bool {
    false
}
