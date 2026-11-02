//! Refuse host-facing helpers during a `netsuke test` manifest load.
//!
//! A test load is deny-by-default: helpers that inspect the host, read the
//! filesystem, run a command, reach the network, or consult the ambient clock
//! are registered as deliberate failures carrying the
//! [`TestModeRefusal`] source. Refusing rather than omitting matters because a
//! missing helper reports `MiniJinja`'s unknown-name error, which reads as a
//! typo in the manifest; a refusal names the operation and tells the author
//! that the capability was withheld.
//!
//! Every stub accepts any positional and keyword arguments. A refusal must be
//! reachable from whatever call shape a real helper documents, and a narrower
//! signature would report an argument-count error instead of the refusal it is
//! meant to deliver.

use std::fmt;

use minijinja::{
    Environment, Error, ErrorKind, State,
    value::{Kwargs, Rest, Value},
};

use crate::localization::{self, keys};

use super::{HelperKind, RestrictedSurface, SURFACE};

/// Why a helper is unavailable under `netsuke test`.
///
/// The refusal is a typed error source so a caller can distinguish "this
/// capability was withheld" from "the manifest used an unknown helper" without
/// matching on diagnostic text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestModeRefusal {
    /// Helper name the manifest called.
    operation: String,
}

impl TestModeRefusal {
    /// Record the helper that a test load refused.
    pub(crate) fn new(operation: impl Into<String>) -> Self {
        Self {
            operation: operation.into(),
        }
    }

    /// Return the helper name this refusal names.
    ///
    /// Public because a caller that loads a manifest under the test surface —
    /// including the corpus differential test — has to decide whether a failed
    /// load was a withheld capability, and the operation name is the data that
    /// answers which one.
    #[must_use]
    pub fn operation(&self) -> &str {
        &self.operation
    }
}

impl fmt::Display for TestModeRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            &localization::message(keys::STDLIB_TEST_MODE_HELPER_UNAVAILABLE)
                .with_arg("operation", self.operation())
                .to_string(),
        )
    }
}

impl std::error::Error for TestModeRefusal {}

/// Register a deliberate failure in place of every refused test helper.
///
/// The loader owns `env()` and `glob()` because whether a load may reach the
/// host is a property of the load; registering them here would overwrite the
/// loader's own decision.
pub(super) fn register_refusals(env: &mut Environment<'_>) {
    for entry in SURFACE
        .iter()
        .filter(|entry| RestrictedSurface::Test.refuses(entry))
        .filter(|entry| !super::is_loader_owned(entry))
    {
        match entry.kind {
            HelperKind::Function => register_function_refusal(env, entry.name),
            HelperKind::Filter => register_filter_refusal(env, entry.name),
            HelperKind::Test => register_test_refusal(env, entry.name),
        }
    }
}

/// Register the `glob()` refusal installed by the loader for a test load.
pub(super) fn register_glob_refusal(env: &mut Environment<'_>) {
    register_function_refusal(env, "glob");
}

/// Register one function name as a refusal reachable from any call shape.
fn register_function_refusal(env: &mut Environment<'_>, name: &'static str) {
    env.add_function(
        name,
        move |_args: Rest<Value>, _kwargs: Kwargs| -> Result<Value, Error> {
            Err(refusal_error(name))
        },
    );
}

/// Register one filter name as a refusal reachable from any call shape.
fn register_filter_refusal(env: &mut Environment<'_>, name: &'static str) {
    env.add_filter(
        name,
        move |_state: &State,
              _value: Value,
              _args: Rest<Value>,
              _kwargs: Kwargs|
              -> Result<Value, Error> { Err(refusal_error(name)) },
    );
}

/// Register one test name as a refusal reachable from any call shape.
fn register_test_refusal(env: &mut Environment<'_>, name: &'static str) {
    env.add_test(
        name,
        move |_value: Value, _args: Rest<Value>, _kwargs: Kwargs| -> Result<bool, Error> {
            Err(refusal_error(name))
        },
    );
}

/// Build the `MiniJinja` error that carries a [`TestModeRefusal`] source.
fn refusal_error(operation: &str) -> Error {
    let refusal = TestModeRefusal::new(operation);
    Error::new(ErrorKind::InvalidOperation, refusal.to_string()).with_source(refusal)
}

/// Register the deny-by-default stdlib surface used by `netsuke test`.
///
/// The surface is deterministic: lexical path filters, collection filters, the
/// Boolean formatter, and `timedelta` are available; `now()` is available only
/// when the caller injected a clock into `config`, because the ambient clock is
/// a host read. Everything else is a refusal.
pub(crate) fn register_for_test(
    env: &mut Environment<'_>,
    config: &crate::stdlib::StdlibConfig,
) -> crate::stdlib::StdlibState {
    super::super::register::register_legacy_boolean_formatter(env);
    super::super::path::register_query_filters(env);
    super::super::collections::register_filters(env);
    super::super::recipe_text::register_filters(env, config.dialect());
    if config.clock().is_system() {
        register_function_refusal(env, "now");
    } else {
        super::super::time::register_now(env, config.clock().clone());
    }
    super::super::time::register_query_functions(env);
    register_refusals(env);
    crate::stdlib::StdlibState::default()
}
