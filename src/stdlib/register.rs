//! Registration entrypoints for wiring stdlib helpers into `MiniJinja`.
//!
//! Hooks file tests, path helpers, collection utilities, time functions,
//! network fetch helpers, command wrappers, and the `which` filter/function
//! into a single environment. The public `register` and
//! `register_with_config` entrypoints are re-exported from `netsuke::stdlib`
//! alongside `StdlibConfig` and `NetworkConfig`.

use super::{
    StdlibConfig, StdlibState, collections, command, network, path, time,
    which::{self, WhichConfig, WorkspaceSkipList},
};
use anyhow::Context;
use camino::Utf8Path;
#[cfg(unix)]
use cap_std::fs::FileTypeExt;
use cap_std::{ambient_authority, fs, fs_utf8::Dir};
use minijinja::{
    Environment, Error, ErrorKind, State,
    value::{Kwargs, Value},
};
use std::sync::Arc;

use crate::localization::{self, keys};

/// A template file test: a registration name paired with a capability file
/// type predicate.
type FileTest = (&'static str, fn(fs::FileType) -> bool);

/// Stable text identifying helpers deliberately unavailable to manifest queries.
const MANIFEST_QUERY_DISABLED_HELPER_MARKER: &str = "is disabled while rendering `netsuke help targets`; manifest queries permit \
     only non-disclosing, side-effect-free template helpers";

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
        .context(localization::message(keys::STDLIB_REGISTER_OPEN_DIR))?;
    let cwd = std::env::current_dir()
        .context(localization::message(keys::STDLIB_REGISTER_RESOLVE_DIR))?;
    let path = camino::Utf8PathBuf::from_path_buf(cwd).map_err(|path| {
        anyhow::anyhow!(
            "{}",
            localization::message(keys::STDLIB_REGISTER_DIR_NON_UTF8)
                .with_arg("path", path.display().to_string())
        )
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
    register_read_only_helpers(env, &config);
    time::register_functions(env);
    let impure = state.impure_flag();
    let (network_config, command_config) = config.into_components();
    network::register_functions(env, Arc::clone(&impure), network_config);
    command::register(env, impure, command_config);
    Ok(state)
}

/// Register helpers suitable for manifest queries that must avoid side effects.
///
/// The registration preserves only lexical path filters, collection helpers,
/// and clock-independent time helpers. It rejects helpers that inspect the
/// host, perform I/O, or invoke commands so consumers can render discovery
/// metadata without disclosing host state.
///
pub(crate) fn register_manifest_query(env: &mut Environment<'_>) -> StdlibState {
    let state = StdlibState::default();
    register_query_helpers(env);
    time::register_query_functions(env);
    register_disabled_query_helpers(env);
    state
}

/// Register helpers that do not execute a command or make a network request.
fn register_read_only_helpers(env: &mut Environment<'_>, config: &StdlibConfig) {
    register_file_tests(env);
    path::register_filters(env, config.home_directory().clone());
    collections::register_filters(env);
    let which_cache_capacity = config.which_cache_capacity();
    let which_skip_dirs = WorkspaceSkipList::from_names(config.workspace_skip_dirs());
    let which_cwd = config
        .workspace_root_path()
        .map(|path| Arc::new(path.to_path_buf()));
    let which_path = config.path_override().cloned();
    let which_config =
        WhichConfig::new(which_cwd, which_path, which_skip_dirs, which_cache_capacity)
            .with_pathext_override(config.pathext_override().cloned());
    which::register(env, which_config);
}

/// Register the allowlisted helpers for manifest discovery queries.
fn register_query_helpers(env: &mut Environment<'_>) {
    path::register_query_filters(env);
    collections::register_filters(env);
}

/// Register deliberate failures for helpers excluded from manifest queries.
fn register_disabled_query_helpers(env: &mut Environment<'_>) {
    register_always_disabled_query_helpers(env);
    register_host_dependent_query_helpers(env);
}

/// Register helpers that are never safe while rendering discovery metadata.
fn register_always_disabled_query_helpers(env: &mut Environment<'_>) {
    env.add_function("env", |_variable: String| -> Result<String, Error> {
        Err(manifest_query_operation_error("env"))
    });
    env.add_function("glob", |_pattern: String| -> Result<Value, Error> {
        Err(manifest_query_operation_error("glob"))
    });
    env.add_function(
        "fetch",
        |_url: String, _kwargs: Kwargs| -> Result<Value, Error> {
            Err(manifest_query_operation_error("fetch"))
        },
    );
    env.add_filter(
        "shell",
        |_state: &State,
         _value: Value,
         _command: String,
         _options: Option<Value>|
         -> Result<Value, Error> { Err(manifest_query_operation_error("shell")) },
    );
    env.add_filter(
        "grep",
        |_state: &State,
         _value: Value,
         _pattern: String,
         _flags: Option<Value>,
         _options: Option<Value>|
         -> Result<Value, Error> { Err(manifest_query_operation_error("grep")) },
    );
    env.add_filter(
        "contents",
        |_value: String, _encoding: Option<String>| -> Result<String, Error> {
            Err(manifest_query_operation_error("contents"))
        },
    );
}

/// Register helpers whose result would disclose host state during a query.
fn register_host_dependent_query_helpers(env: &mut Environment<'_>) {
    env.add_filter(
        "which",
        |_value: Value, _kwargs: Kwargs| -> Result<Value, Error> {
            Err(manifest_query_operation_error("which"))
        },
    );
    env.add_function(
        "which",
        |_value: Value, _kwargs: Kwargs| -> Result<Value, Error> {
            Err(manifest_query_operation_error("which"))
        },
    );
    env.add_function(
        "command_available",
        |_value: Value, _kwargs: Kwargs| -> Result<bool, Error> {
            Err(manifest_query_operation_error("command_available"))
        },
    );
    env.add_function("now", |_kwargs: Kwargs| -> Result<Value, Error> {
        Err(manifest_query_operation_error("now"))
    });
    env.add_filter("realpath", |_value: String| -> Result<String, Error> {
        Err(manifest_query_operation_error("realpath"))
    });
    env.add_filter("expanduser", |_value: String| -> Result<String, Error> {
        Err(manifest_query_operation_error("expanduser"))
    });
    env.add_filter("size", |_value: String| -> Result<u64, Error> {
        Err(manifest_query_operation_error("size"))
    });
    env.add_filter("linecount", |_value: String| -> Result<usize, Error> {
        Err(manifest_query_operation_error("linecount"))
    });
    env.add_filter(
        "hash",
        |_value: String, _algorithm: Option<String>| -> Result<String, Error> {
            Err(manifest_query_operation_error("hash"))
        },
    );
    env.add_filter(
        "digest",
        |_value: String,
         _length: Option<usize>,
         _algorithm: Option<String>|
         -> Result<String, Error> { Err(manifest_query_operation_error("digest")) },
    );
}

/// Explain why a restricted helper is unavailable while querying a manifest.
fn manifest_query_operation_error(operation: &str) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!("{operation} {MANIFEST_QUERY_DISABLED_HELPER_MARKER}"),
    )
}

/// Return whether an error marks a helper intentionally unavailable in queries.
pub(crate) fn is_manifest_query_disabled_error(error: &Error) -> bool {
    error.kind() == ErrorKind::InvalidOperation
        && error
            .to_string()
            .contains(MANIFEST_QUERY_DISABLED_HELPER_MARKER)
}

/// Convert UTF-8 or fall back to bytes for byte-oriented network helpers.
#[must_use]
pub fn value_from_bytes(bytes: Vec<u8>) -> Value {
    match String::from_utf8(bytes) {
        Ok(text) => Value::from(text),
        Err(err) => Value::from_bytes(err.into_bytes()),
    }
}

/// The file tests registered as template tests on Unix.
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

/// The file tests registered as template tests on non-Unix platforms.
#[cfg(not(unix))]
const FILE_TESTS: &[FileTest] = &[
    ("dir", is_dir),
    ("file", is_file),
    ("symlink", is_symlink),
    ("pipe", is_fifo),
    ("block_device", is_block_device),
    ("char_device", is_char_device),
    ("device", is_device),
];

/// Register the `is <kind>` file tests, treating non-string inputs as a
/// negative match.
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

/// Test whether a file type is a directory.
fn is_dir(ft: fs::FileType) -> bool {
    ft.is_dir()
}
/// Test whether a file type is a regular file.
fn is_file(ft: fs::FileType) -> bool {
    ft.is_file()
}
/// Test whether a file type is a symbolic link.
fn is_symlink(ft: fs::FileType) -> bool {
    ft.is_symlink()
}

/// Test whether a file type is a named pipe (FIFO).
#[cfg(unix)]
fn is_fifo(ft: fs::FileType) -> bool {
    ft.is_fifo()
}

/// Report whether a file type is a named pipe, always `false` off Unix.
#[cfg(not(unix))]
const fn is_fifo(_ft: fs::FileType) -> bool {
    false
}

/// Test whether a file type is a block device.
#[cfg(unix)]
fn is_block_device(ft: fs::FileType) -> bool {
    ft.is_block_device()
}

/// Report whether a file type is a block device, always `false` off Unix.
#[cfg(not(unix))]
const fn is_block_device(_ft: fs::FileType) -> bool {
    false
}

/// Test whether a file type is a character device.
#[cfg(unix)]
fn is_char_device(ft: fs::FileType) -> bool {
    ft.is_char_device()
}

/// Report whether a file type is a character device, always `false` off Unix.
#[cfg(not(unix))]
const fn is_char_device(_ft: fs::FileType) -> bool {
    false
}

/// Test whether a file type is a block or character device.
#[cfg(unix)]
fn is_device(ft: fs::FileType) -> bool {
    is_block_device(ft) || is_char_device(ft)
}

/// Report whether a file type is a device, always `false` off Unix.
#[cfg(not(unix))]
const fn is_device(_ft: fs::FileType) -> bool {
    false
}
