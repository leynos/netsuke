//! Registration entrypoints for wiring stdlib helpers into `MiniJinja`.
//!
//! Hooks file tests, path helpers, collection utilities, time functions,
//! network fetch helpers, command wrappers, and the `which` filter/function
//! into a single environment. The public `register` and
//! `register_with_config` entrypoints are re-exported from `netsuke::stdlib`
//! alongside `StdlibConfig` and `NetworkConfig`.

use super::{StdlibConfig, StdlibState, collections, command, network, path, time, which};
use anyhow::Context;
use camino::Utf8Path;
#[cfg(unix)]
use cap_std::fs::FileTypeExt;
use cap_std::{ambient_authority, fs, fs_utf8::Dir};
use minijinja::{Environment, Error, value::Value};
use std::sync::Arc;

type FileTest = (&'static str, fn(fs::FileType) -> bool);

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
/// capability-based I/O (for example, when permissions are insufficient or the
/// directory no longer exists) or when the current directory path contains
/// non-UTF-8 components and cannot be converted into a UTF-8 workspace root.
pub fn register(env: &mut Environment<'_>) -> anyhow::Result<StdlibState> {
    let root = Dir::open_ambient_dir(".", ambient_authority())
        .context("open current directory for stdlib registration")?;
    let cwd =
        std::env::current_dir().context("resolve current directory for stdlib registration")?;
    let path = camino::Utf8PathBuf::from_path_buf(cwd).map_err(|path| {
        anyhow::anyhow!("current directory contains non-UTF-8 components: {path:?}")
    })?;
    register_with_config(
        env,
        StdlibConfig::new(root)?.with_workspace_root_path(path)?,
    )
}

/// Register stdlib helpers using an explicit configuration.
///
/// This is intended for callers that have already derived a capability-scoped
/// workspace directory and need to wire the stdlib into a `MiniJinja`
/// environment.
///
/// # Examples
///
/// ```rust,no_run
/// use cap_std::{ambient_authority, fs_utf8::Dir};
/// use minijinja::Environment;
/// use netsuke::stdlib::{self, StdlibConfig};
///
/// let dir = Dir::open_ambient_dir(".", ambient_authority())
///     .expect("open workspace");
/// let mut env = Environment::new();
/// let config = StdlibConfig::new(dir).expect("configure stdlib workspace");
/// let _state = stdlib::register_with_config(&mut env, config);
/// ```
///
/// # Errors
///
/// Returns an error if stdlib components cannot be registered (for example,
/// when the which resolver cache configuration is invalid).
pub fn register_with_config(
    env: &mut Environment<'_>,
    config: StdlibConfig,
) -> anyhow::Result<StdlibState> {
    let state = StdlibState::default();
    register_file_tests(env);
    path::register_filters(env);
    collections::register_filters(env);
    let which_cwd = config
        .workspace_root_path()
        .map(|path| Arc::new(path.to_path_buf()));
    which::register(env, which_cwd)?;
    let impure = state.impure_flag();
    let (network_config, command_config) = config.into_components();
    network::register_functions(env, Arc::clone(&impure), network_config);
    command::register(env, impure, command_config);
    time::register_functions(env);
    Ok(state)
}

/// Convert UTF-8 or fall back to bytes for byte-oriented network helpers.
#[must_use]
pub fn value_from_bytes(bytes: Vec<u8>) -> Value {
    match String::from_utf8(bytes) {
        Ok(text) => Value::from(text),
        Err(err) => Value::from_bytes(err.into_bytes()),
    }
}

#[cfg(unix)]
const FILE_TESTS: &[FileTest] = &[
    ("dir", is_dir),
    ("file", is_file),
    ("symlink", is_symlink),
    ("pipe", is_fifo),
    ("block_device", is_block_device),
    ("char_device", is_char_device),
    ("device", is_device),
];

#[cfg(not(unix))]
const FILE_TESTS: &[FileTest] = &[("dir", is_dir), ("file", is_file), ("symlink", is_symlink)];

fn register_file_tests(env: &mut Environment<'_>) {
    for &(name, pred) in FILE_TESTS {
        env.add_test(name, move |val: Value| -> Result<bool, Error> {
            if let Some(s) = val.as_str() {
                return path::file_type_matches(Utf8Path::new(s), pred);
            }
            // Treat non-string inputs as a negative match to mirror MiniJinja's
            // permissive truthiness semantics (for example `42 is odd` yields
            // `false` rather than raising a type error).
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

#[cfg(unix)]
fn is_block_device(ft: fs::FileType) -> bool {
    ft.is_block_device()
}

#[cfg(unix)]
fn is_char_device(ft: fs::FileType) -> bool {
    ft.is_char_device()
}

#[cfg(unix)]
fn is_device(ft: fs::FileType) -> bool {
    is_block_device(ft) || is_char_device(ft)
}
