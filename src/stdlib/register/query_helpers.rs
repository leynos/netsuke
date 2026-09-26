//! Deliberate failures for helpers excluded from manifest queries.
//!
//! Manifest discovery renders `netsuke help targets`, which must not disclose
//! host state or perform side effects. Every helper that is legitimate at build
//! time but unsafe to evaluate while discovering is registered here as a stub
//! that fails with a recognizable marker, so the failure is a clear diagnostic
//! rather than an undefined-function error.
//!
//! This is a `#[path]`-declared child of `register.rs` rather than an inline
//! module: the parent sits at AGENTS.md's 400-line cap, and the query-disabled
//! cluster is the natural seam because it shares one vocabulary — the marker
//! constant, the error constructor that appends it, and the two registration
//! functions that raise it — and nothing else in the parent refers to any of
//! them.

use super::MANIFEST_QUERY_DISABLED_HELPER_MARKER;
use minijinja::{
    Environment, Error, ErrorKind, State,
    value::{Kwargs, Value},
};

/// Register deliberate failures for helpers excluded from manifest queries.
pub(super) fn register_disabled_query_helpers(env: &mut Environment<'_>) {
    register_always_disabled_query_helpers(env);
    register_host_dependent_query_helpers(env);
}

/// Register helpers that are never safe while rendering discovery metadata.
fn register_always_disabled_query_helpers(env: &mut Environment<'_>) {
    env.add_function(
        "env",
        |_variable: String, _kwargs: Kwargs| -> Result<String, Error> {
            Err(manifest_query_operation_error("env"))
        },
    );
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
    env.add_filter("linecount", |_value: String| -> Result<u64, Error> {
        Err(manifest_query_operation_error("linecount"))
    });
    env.add_function(
        "hash",
        |_value: Value, _kwargs: Kwargs| -> Result<String, Error> {
            Err(manifest_query_operation_error("hash"))
        },
    );
    env.add_filter(
        "digest",
        |_state: &State,
         _value: Value,
         _algorithm: String,
         _encoding: Option<String>|
         -> Result<String, Error> { Err(manifest_query_operation_error("digest")) },
    );
}

/// Explain why a restricted helper is unavailable while querying a manifest.
pub(super) fn manifest_query_operation_error(operation: &str) -> Error {
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
