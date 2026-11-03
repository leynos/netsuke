//! Registration entrypoints for wiring stdlib helpers into `MiniJinja`.
//!
//! Hooks file tests, path helpers, collection utilities, time functions,
//! network fetch helpers, command wrappers, and the `which` filter/function
//! into a single environment. The public `register` and
//! `register_with_config` entrypoints are re-exported from `netsuke::stdlib`
//! alongside `StdlibConfig` and `NetworkConfig`.

use super::{
    StdlibConfig, StdlibState, collections, command, network, path, recipe_text,
    restricted_helpers, time,
    which::{self, WhichConfig, WorkspaceSkipList},
};
use anyhow::Context;
use camino::Utf8Path;
#[cfg(unix)]
use cap_std::fs::FileTypeExt;
use cap_std::{ambient_authority, fs, fs_utf8::Dir};
use minijinja::{
    Environment, Error, escape_formatter,
    value::{Value, ValueKind},
};
use std::sync::Arc;

use crate::localization::{self, keys};
use crate::shell_word::ShellDialect;

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
    register_legacy_boolean_formatter(env);
    let state = StdlibState::default();
    register_read_only_helpers(env, &config);
    let clock = config.clock().clone();
    tracing::debug!(
        clock_source = clock.source_label(),
        "registered stdlib time helpers"
    );
    time::register_functions(env, clock);
    let impure = state.impure_flag();
    let (network_config, file_config, command_config) = config.into_components();
    network::register_functions(env, Arc::clone(&impure), network_config);
    tracing::debug!(
        file_max_read_bytes = file_config.max_read_bytes,
        "registered stdlib file-reading filters"
    );
    command::register(env, impure, command_config);
    Ok(state)
}

/// Preserve lowercase Boolean interpolation for existing manifests.
///
/// The test surface installs the same formatter, so a manifest that renders a
/// Boolean produces identical text under both surfaces.
pub(super) fn register_legacy_boolean_formatter(env: &mut Environment<'_>) {
    env.set_formatter(|out, state, value| {
        if value.kind() == ValueKind::Bool {
            out.write_str(if value.is_true() { "true" } else { "false" })
                .map_err(Error::from)
        } else {
            escape_formatter(out, state, value)
        }
    });
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
    restricted_helpers::register_query_refusals(env);
    state
}

/// Register helpers that do not execute a command or make a network request.
fn register_read_only_helpers(env: &mut Environment<'_>, config: &StdlibConfig) {
    register_file_tests(env);
    path::register_filters(
        env,
        config.home_directory().clone(),
        config.file_max_read_bytes(),
    );
    collections::register_filters(env);
    recipe_text::register_filters(env, config.dialect());
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
///
/// Query metadata uses a fixed POSIX quoting dialect so its rendered values do
/// not depend on the host or on build-shell configuration. Query metadata is
/// never executed; full manifest rendering continues to use the selected
/// recipe shell.
fn register_query_helpers(env: &mut Environment<'_>) {
    path::register_query_filters(env);
    collections::register_filters(env);
    recipe_text::register_filters(env, ShellDialect::Sh);
}

/// Convert UTF-8 or fall back to bytes for byte-oriented network helpers.
#[must_use]
pub fn value_from_bytes(bytes: Vec<u8>) -> Value {
    match String::from_utf8(bytes) {
        Ok(text) => Value::from(text),
        Err(err) => Value::from_bytes(err.into_bytes()),
    }
}

/// Register inventoried `is <kind>` file tests, treating non-string inputs as
/// a negative match.
fn register_file_tests(env: &mut Environment<'_>) {
    for name in restricted_helpers::file_test_names() {
        let Some(pred) = file_test_predicate(name) else {
            tracing::error!(
                file_test = name,
                "file-test inventory has no build predicate"
            );
            continue;
        };
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

/// Return the build-time predicate for an inventoried file-test name.
fn file_test_predicate(name: &str) -> Option<fn(fs::FileType) -> bool> {
    match name {
        "dir" => Some(is_dir),
        "file" => Some(is_file),
        "symlink" => Some(is_symlink),
        "pipe" => Some(is_fifo),
        "block_device" => Some(is_block_device),
        "char_device" => Some(is_char_device),
        "device" => Some(is_device),
        _ => None,
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

#[cfg(test)]
mod tests {
    //! Guard the inventory-to-dispatch mapping this module owns.
    //!
    //! `register_file_tests` skips an inventoried name whose predicate is
    //! missing, logging rather than failing. The skip keeps the build surface
    //! running, but it would also let the inventory and the dispatch table drift
    //! apart silently, so the drift is asserted here instead.

    use super::{file_test_predicate, restricted_helpers::file_test_names};

    // A gap names the offending helper rather than a count, because the
    // registration row that must change is not discoverable from a total.
    #[test]
    fn every_inventoried_file_test_has_a_build_predicate() {
        let unmapped: Vec<&str> = file_test_names()
            .filter(|name| file_test_predicate(name).is_none())
            .collect();
        assert!(
            unmapped.is_empty(),
            "these inventoried file tests have no `file_test_predicate` arm: {unmapped:?}"
        );
    }
}
