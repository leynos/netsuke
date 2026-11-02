//! Register query refusals without changing their existing call contracts.

use minijinja::{
    Environment, Error, ErrorKind, State,
    value::{Kwargs, Value},
};

use super::{HelperKind, RestrictedSurface, SURFACE, is_loader_owned};

/// Marker retained in query-refusal text so the Jinja boundary can classify it.
const MANIFEST_QUERY_DISABLED_HELPER_MARKER: &str = concat!(
    "is disabled while rendering `netsuke help targets`; manifest queries permit ",
    "only non-disclosing, side-effect-free template helpers"
);

/// Register the deliberate failures selected by the query disposition table.
///
/// Host-reading helpers are skipped: whether a load may reach the host is a
/// property of the load, so the loader registers those two from its own
/// disposition. Registering them here too would overwrite the loader's
/// decision, and a test load would lose a real surface reader to a refusal.
pub(super) fn register_refusals(env: &mut Environment<'_>, surface: RestrictedSurface) {
    for entry in SURFACE
        .iter()
        .filter(|entry| surface.refuses(entry))
        .filter(|entry| !is_loader_owned(entry))
    {
        match entry.kind {
            HelperKind::Function => register_query_function_stub(env, entry.name),
            HelperKind::Filter => register_query_filter_stub(env, entry.name),
            HelperKind::Test => register_query_test_stub(env, entry.name),
        }
    }
}

/// Register only the `env()` refusal for a load that may not read the host.
pub(crate) fn register_env_refusal(env: &mut Environment<'_>) {
    register_query_function_stub(env, "env");
}

/// Register only the `glob()` refusal for a load that may not touch the
/// filesystem.
pub(super) fn register_glob_refusal(env: &mut Environment<'_>) {
    register_query_function_stub(env, "glob");
}

/// Register a query-refusing function with its established argument shape.
fn register_query_function_stub(env: &mut Environment<'_>, name: &'static str) {
    match name {
        "env" => env.add_function(
            name,
            move |_variable: String, _kwargs: Kwargs| -> Result<String, Error> {
                Err(manifest_query_operation_error(name))
            },
        ),
        "glob" => env.add_function(name, move |_pattern: String| -> Result<Value, Error> {
            Err(manifest_query_operation_error(name))
        }),
        "fetch" => env.add_function(
            name,
            move |_url: String, _kwargs: Kwargs| -> Result<Value, Error> {
                Err(manifest_query_operation_error(name))
            },
        ),
        "which" => env.add_function(
            name,
            move |_value: Value, _kwargs: Kwargs| -> Result<Value, Error> {
                Err(manifest_query_operation_error(name))
            },
        ),
        "command_available" => env.add_function(
            name,
            move |_value: Value, _kwargs: Kwargs| -> Result<bool, Error> {
                Err(manifest_query_operation_error(name))
            },
        ),
        "now" => env.add_function(name, move |_kwargs: Kwargs| -> Result<Value, Error> {
            Err(manifest_query_operation_error(name))
        }),
        _ => {}
    }
}

/// Register a query-refusing filter with its established argument shape.
fn register_query_filter_stub(env: &mut Environment<'_>, name: &'static str) {
    match name {
        "which" => env.add_filter(
            name,
            move |_value: Value, _kwargs: Kwargs| -> Result<Value, Error> {
                Err(manifest_query_operation_error(name))
            },
        ),
        "shell" => env.add_filter(
            name,
            move |_state: &State,
                  _value: Value,
                  _command: String,
                  _options: Option<Value>|
                  -> Result<Value, Error> { Err(manifest_query_operation_error(name)) },
        ),
        "grep" => env.add_filter(
            name,
            move |_state: &State,
                  _value: Value,
                  _pattern: String,
                  _flags: Option<Value>,
                  _options: Option<Value>|
                  -> Result<Value, Error> { Err(manifest_query_operation_error(name)) },
        ),
        "contents" => env.add_filter(
            name,
            move |_value: String, _encoding: Option<String>| -> Result<String, Error> {
                Err(manifest_query_operation_error(name))
            },
        ),
        "realpath" | "expanduser" => {
            env.add_filter(name, move |_value: String| -> Result<String, Error> {
                Err(manifest_query_operation_error(name))
            });
        }
        "size" => env.add_filter(name, move |_value: String| -> Result<u64, Error> {
            Err(manifest_query_operation_error(name))
        }),
        "linecount" => env.add_filter(name, move |_value: String| -> Result<usize, Error> {
            Err(manifest_query_operation_error(name))
        }),
        "hash" => env.add_filter(
            name,
            move |_value: String, _algorithm: Option<String>| -> Result<String, Error> {
                Err(manifest_query_operation_error(name))
            },
        ),
        "digest" => env.add_filter(
            name,
            move |_value: String,
                  _length: Option<usize>,
                  _algorithm: Option<String>|
                  -> Result<String, Error> {
                Err(manifest_query_operation_error(name))
            },
        ),
        _ => {}
    }
}

/// Register query-refusing tests using the single inventory of test names.
fn register_query_test_stub(env: &mut Environment<'_>, name: &'static str) {
    env.add_test(name, move |_value: Value| -> Result<bool, Error> {
        Err(manifest_query_operation_error(name))
    });
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
