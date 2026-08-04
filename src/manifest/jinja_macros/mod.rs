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
use minijinja::{
    Environment, Error, State,
    value::{Kwargs, Value},
};
use serde::Serialize;

mod invocation;

use invocation::{make_macro_fn, validate_macro};

const MACRO_IMPORTS_GLOBAL: &str = "__netsuke_manifest_macro_imports";

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
    env.add_function(
        name.clone(),
        make_macro_fn(template_name.clone(), name.clone()),
    );
    register_macro_import(env, &template_name, &name);
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
pub(crate) fn render_template(
    env: &Environment,
    template: &str,
    context: &impl Serialize,
) -> Result<String, Error> {
    let Some(imports) = macro_imports(env) else {
        return env.render_str(template, context);
    };
    env.render_str(&[imports.as_str(), template].concat(), context)
}

fn register_macro_import(env: &mut Environment<'static>, template_name: &str, macro_name: &str) {
    let existing = macro_imports(env).unwrap_or_default();
    let import = format!("{{% from '{template_name}' import {macro_name} %}}");
    env.add_global(MACRO_IMPORTS_GLOBAL, [existing, import].concat());
}

fn macro_imports(env: &Environment) -> Option<String> {
    env.globals().find_map(|(name, value)| {
        (name == MACRO_IMPORTS_GLOBAL)
            .then(|| value.as_str().map(str::to_owned))
            .flatten()
    })
}

/// Invoke a `MiniJinja` value with optional keyword arguments.
///
/// `MiniJinja` encodes keyword arguments by appending a [`Kwargs`] value to the
/// positional slice. This helper hides that convention so callers can pass the
/// keyword collection explicitly.
///
/// # Examples
///
/// ```rust,ignore
/// use minijinja::{Environment, value::{Kwargs, Value}};
/// use netsuke::manifest::jinja_macros::call_macro_value;
///
/// let mut env = Environment::new();
/// env.add_template(
///     "macro",
///     "{% macro greet(name='friend') %}hi {{ name }}{% endmacro %}",
/// )
/// .unwrap();
/// let template = env.get_template("macro").unwrap();
/// let captured = template.render_captured(()).unwrap();
/// let value = captured.state().lookup("greet").unwrap();
/// let kwargs = Kwargs::from_iter([(String::from("name"), Value::from("Ada"))]);
/// let rendered = call_macro_value(captured.state(), &value, &[], Some(kwargs)).unwrap();
/// assert_eq!(rendered.to_string(), "hi Ada");
/// ```
pub(crate) fn call_macro_value(
    state: &State,
    macro_value: &Value,
    positional: &[Value],
    kwargs: Option<Kwargs>,
) -> Result<Value, Error> {
    kwargs.map_or_else(
        || macro_value.call(state, positional),
        |macro_kwargs| {
            let mut call_args = Vec::with_capacity(positional.len() + 1);
            call_args.extend_from_slice(positional);
            call_args.push(Value::from(macro_kwargs));
            macro_value.call(state, call_args.as_slice())
        },
    )
}
