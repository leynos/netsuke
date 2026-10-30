//! Classify `MiniJinja` failures at the manifest-query boundary.
//!
//! `netsuke help targets` renders `when` conditions against a stdlib surface
//! whose host-observing helpers are deliberate refusals. Expansion must tell a
//! refused helper apart from a genuine render failure without seeing the
//! stdlib's error vocabulary. This module owns that outcome and the single
//! translation into it.
//!
//! It is a child module rather than part of `jinja_macros/mod.rs` for two
//! reasons. The parent sits at AGENTS.md's 400-line cap, and this boundary has
//! a different subject from macro registration: only this code must recognize
//! the restricted-helper marker, while the rest of the macro machinery only
//! evaluates expressions. Keeping [`QueryEvaluation`] and the adapter together
//! means a change to either half is reviewed in one place.

use anyhow::Result;
use minijinja::Error;

/// Represents a Jinja evaluation that may be disabled in manifest-query mode.
///
/// This preserves the distinction between a false condition and a condition
/// that discovery cannot safely inspect, without leaking `MiniJinja` errors
/// into manifest expansion.
pub(crate) enum QueryEvaluation<T> {
    /// Holds a successfully evaluated value.
    Value(T),
    /// Marks an expression that invoked a query-disabled helper.
    QueryDisabled,
}

/// Convert a `MiniJinja` result into the query-safe evaluation boundary.
///
/// The stdlib marks intentionally disabled helpers with a stable `MiniJinja`
/// error. This adapter is the only place that turns that marker into
/// [`QueryEvaluation::QueryDisabled`], so expansion never matches on the
/// stdlib's error text. `evaluate_when_expression` and `render_when_template`
/// call it; keeping it beside [`QueryEvaluation`] keeps the boundary's two
/// halves together.
///
/// # Errors
///
/// Returns an error when the failure is not the query-refusal marker, so a
/// genuine render failure still surfaces with its own context.
pub(super) fn classify_query_evaluation<T>(
    evaluation: std::result::Result<T, Error>,
) -> Result<QueryEvaluation<T>> {
    match evaluation {
        Ok(value) => Ok(QueryEvaluation::Value(value)),
        Err(error) if crate::stdlib::is_manifest_query_disabled_error(&error) => {
            Ok(QueryEvaluation::QueryDisabled)
        }
        Err(error) => Err(error.into()),
    }
}

impl<T> QueryEvaluation<T> {
    /// Transform a successful value while preserving query-disabled state.
    pub(super) fn map<U>(self, transform: impl FnOnce(T) -> U) -> QueryEvaluation<U> {
        match self {
            Self::Value(value) => QueryEvaluation::Value(transform(value)),
            Self::QueryDisabled => QueryEvaluation::QueryDisabled,
        }
    }
}
