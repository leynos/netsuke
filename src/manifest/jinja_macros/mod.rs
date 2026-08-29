//! Helpers for registering and invoking manifest-defined Jinja macros.
//!
//! The manifest can define reusable macros via the `macros` section. This
//! module compiles those macros into standalone templates and exposes them to
//! the main rendering environment so manifest templates can invoke them like
//! built-in helpers.

use super::ManifestValue;
use crate::ast::MacroDefinition;
use crate::localization::{self, keys};
use anyhow::{Context, Result};
use minijinja::{Environment, Error, value::Value};
use serde::Serialize;

mod call;
mod invocation;
mod telemetry;

// Only the manifest test suite reaches the helper through the parent path;
// `invocation` imports it from the sibling module directly.
#[cfg(test)]
pub(crate) use call::call_macro_value;
use invocation::{make_macro_fn, validate_macro};

/// Global name holding accumulated import statements for manifest macros.
const MACRO_IMPORTS_GLOBAL: &str = "__netsuke_manifest_macro_imports";

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

/// Evaluate a `when` expression when it has expression syntax.
///
/// Returns `None` when `MiniJinja` cannot compile expression syntax, allowing
/// the caller to preserve the established template-render fallback.
///
/// # Errors
///
/// Returns an error when a compiled expression fails for a reason other than
/// invoking a helper disabled during manifest queries.
pub(crate) fn evaluate_when_expression(
    env: &Environment,
    expression: &str,
    context: &Value,
) -> Result<Option<QueryEvaluation<bool>>> {
    let Ok(compiled) = env.compile_expression(expression) else {
        return Ok(None);
    };
    classify_query_evaluation(compiled.eval(context))
        .map(|evaluation| evaluation.map(|value| value.is_true()))
        .with_context(|| {
            localization::message(keys::MANIFEST_WHEN_EVAL_ERROR).with_arg("expr", expression)
        })
        .map(Some)
}

/// Render a template-form `when` condition with query-disabled classification.
///
/// # Errors
///
/// Returns an error when template rendering fails for a reason other than a
/// helper disabled during manifest queries.
pub(crate) fn render_when_template(
    env: &Environment,
    template: &str,
    context: &Value,
) -> Result<QueryEvaluation<String>> {
    classify_query_evaluation(render_template(env, template, context)).with_context(|| {
        localization::message(keys::MANIFEST_WHEN_TEMPLATE_ERROR).with_arg("expr", template)
    })
}

/// Convert a `MiniJinja` result into the query-safe evaluation boundary.
///
/// The stdlib marks intentionally disabled helpers with a stable `MiniJinja`
/// error. Keeping the adapter here confines that implementation detail to the
/// Jinja boundary; manifest expansion consumes only [`QueryEvaluation`].
fn classify_query_evaluation<T>(
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
    fn map<U>(self, transform: impl FnOnce(T) -> U) -> QueryEvaluation<U> {
        match self {
            Self::Value(value) => QueryEvaluation::Value(transform(value)),
            Self::QueryDisabled => QueryEvaluation::QueryDisabled,
        }
    }
}

/// Extract the macro identifier from a signature string.
///
/// The signature must follow the form `name(params)` where `name` is a valid
/// Jinja identifier and `params` is a parameter list (possibly empty).
///
/// # Errors
///
/// Returns an error if the signature is empty, lacks a parameter list, or the
/// identifier before `(` is empty.
///
/// # Examples
///
/// ```rust,ignore
/// let name = parse_macro_name("greet(name)").expect("valid signature");
/// assert_eq!(name, "greet");
/// ```
pub(crate) fn parse_macro_name(signature: &str) -> Result<String> {
    let trimmed = signature.trim();
    if trimmed.is_empty() {
        return Err(anyhow::anyhow!(
            "{}",
            localization::message(keys::MANIFEST_MACRO_SIGNATURE_MISSING_IDENTIFIER)
                .with_arg("signature", signature)
        ));
    }
    let Some((name_segment, _rest)) = trimmed.split_once('(') else {
        return Err(anyhow::anyhow!(
            "{}",
            localization::message(keys::MANIFEST_MACRO_SIGNATURE_MISSING_PARAMS)
                .with_arg("signature", signature)
        ));
    };
    let identifier = name_segment.trim();
    if identifier.is_empty() {
        return Err(anyhow::anyhow!(
            "{}",
            localization::message(keys::MANIFEST_MACRO_SIGNATURE_MISSING_IDENTIFIER)
                .with_arg("signature", signature)
        ));
    }
    Ok(identifier.to_owned())
}

/// Register a single manifest macro in the Jinja environment.
///
/// Compiles the macro body into a template and registers a callable function
/// with the extracted macro name. The template name is synthesised using the
/// provided index to ensure uniqueness.
///
/// # Errors
///
/// Returns an error if the macro signature is invalid or template compilation
/// fails.
pub(crate) fn register_macro(
    env: &mut Environment<'static>,
    macro_def: &MacroDefinition,
    index: usize,
) -> Result<()> {
    let name = parse_macro_name(&macro_def.signature)?;
    let template_name = format!("__manifest_macro_{index}_{name}");
    let template_source = format!(
        "{{% macro {} %}}{}{{% endmacro %}}",
        macro_def.signature, macro_def.body
    );

    env.add_template_owned(template_name.clone(), template_source)
        .with_context(|| {
            localization::message(keys::MANIFEST_MACRO_COMPILE_FAILED).with_arg("name", &name)
        })?;

    validate_macro(env, &template_name, &name)?;
    register_macro_import(env, &template_name, &name);
    env.add_function(name.clone(), make_macro_fn(template_name, name));
    Ok(())
}

/// Register all manifest macros from a YAML document.
///
/// Expects the YAML to have a `macros` key containing a sequence of mappings,
/// each with `signature` and `body` string fields. Registers each macro in the
/// environment using [`register_macro`].
///
/// # Errors
///
/// Returns an error if the YAML shape is invalid, any macro signature is
/// malformed, or template compilation fails.
pub(crate) fn register_manifest_macros(
    doc: &ManifestValue,
    env: &mut Environment<'static>,
) -> Result<()> {
    let Some(macros) = doc.get("macros").cloned() else {
        return Ok(());
    };

    let defs: Vec<MacroDefinition> = serde_json::from_value(macros)
        .context(localization::message(keys::MANIFEST_MACRO_SEQUENCE_INVALID))?;

    for (idx, def) in defs.iter().enumerate() {
        register_macro(env, def, idx).with_context(|| {
            localization::message(keys::MANIFEST_MACRO_REGISTER_FAILED)
                .with_arg("signature", &def.signature)
        })?;
    }
    Ok(())
}

/// Render a manifest template with all registered manifest macros imported.
///
/// Imports place macro values in the active template state, which preserves
/// Jinja caller-block context without raw pointers or extended lifetimes.
///
/// Renders are traced and metered with bounded data only: the outcome, whether
/// macro imports were present, and — on failure — the `MiniJinja` error kind.
/// Template text, macro names, and context values never reach telemetry.
pub(crate) fn render_template(
    env: &Environment,
    template: &str,
    context: &impl Serialize,
) -> Result<String, Error> {
    let imports = macro_imports(env);
    let has_macro_imports = imports.is_some();
    telemetry::instrument_template_render(has_macro_imports, || {
        imports.map_or_else(
            || env.render_str(template, context),
            |import_block| env.render_str(&[import_block.as_str(), template].concat(), context),
        )
    })
}

/// Append a `from ... import` statement for one macro to the import global.
fn register_macro_import(env: &mut Environment<'static>, template_name: &str, macro_name: &str) {
    let existing = macro_imports(env).unwrap_or_default();
    let import = format!("{{% from '{template_name}' import {macro_name} %}}");
    env.add_global(MACRO_IMPORTS_GLOBAL, [existing, import].concat());
}

/// Read the accumulated macro-import statements from the environment, if any.
fn macro_imports(env: &Environment) -> Option<String> {
    env.globals().find_map(|(name, value)| {
        (name == MACRO_IMPORTS_GLOBAL)
            .then(|| value.as_str().map(str::to_owned))
            .flatten()
    })
}
