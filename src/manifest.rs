//! Manifest loading helpers.
//!
//! This module parses a `Netsukefile` without relying on a global Jinja
//! preprocessing pass. The YAML is parsed first and Jinja expressions are
//! evaluated only within string values or the `foreach` and `when` keys. It
//! exposes `env()` to read environment variables and `glob()` to expand
//! filesystem patterns during template evaluation. Both helpers fail fast when
//! inputs are missing or patterns are invalid.

use crate::ast::NetsukeManifest;
use anyhow::{Context, Result};
use minijinja::{
    Environment, Error, ErrorKind, State, UndefinedBehavior,
    value::{Rest, Value},
};
use serde_yml::Value as YamlValue;
use std::{fs, path::Path};

mod diagnostics;
mod expand;
mod glob;
mod hints;
mod render;

pub use diagnostics::{ManifestError, map_yaml_error};
pub use glob::glob_paths;

pub use expand::expand_foreach;
pub use render::render_manifest;

/// Resolve the value of an environment variable for the `env()` Jinja helper.
///
/// Returns the variable's value or a structured error that mirrors Jinja's
/// failure modes, ensuring templates halt when a variable is missing or not
/// valid UTF-8.
///
/// # Examples
///
/// The [`EnvLock`](test_support::env_lock::EnvLock) guard serialises access to
/// the process environment so tests do not interfere with each other.
///
/// ```rust,ignore
/// use test_support::env_lock::EnvLock;
/// let _guard = EnvLock::acquire();
/// std::env::set_var("FOO", "bar");
/// assert_eq!(env("FOO").unwrap(), "bar");
/// std::env::remove_var("FOO");
/// ```
fn env_var(name: &str) -> std::result::Result<String, Error> {
    match std::env::var(name) {
        Ok(val) => Ok(val),
        Err(std::env::VarError::NotPresent) => Err(Error::new(
            ErrorKind::UndefinedError,
            format!("environment variable '{name}' is not set"),
        )),
        Err(std::env::VarError::NotUnicode(_)) => Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("environment variable '{name}' is set but contains invalid UTF-8"),
        )),
    }
}

fn parse_macro_name(signature: &str) -> Result<String> {
    let trimmed = signature.trim();
    let Some((name, _rest)) = trimmed.split_once('(') else {
        return Err(anyhow::anyhow!(
            "macro signature '{signature}' must include parameter list"
        ));
    };
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow::anyhow!(
            "macro signature '{signature}' is missing an identifier"
        ));
    }
    Ok(name.to_string())
}

fn register_macro(env: &mut Environment, signature: &str, body: &str, index: usize) -> Result<()> {
    let name = parse_macro_name(signature)?;
    let template_name = format!("__manifest_macro_{index}_{name}");
    let template_source = format!("{{% macro {signature} %}}{body}{{% endmacro %}}",);

    env.add_template_owned(template_name.clone(), template_source)
        .with_context(|| format!("compile macro '{name}'"))?;

    let macro_template = template_name.clone();
    let macro_name = name.clone();
    env.add_function(
        name,
        move |state: &State, args: Rest<Value>| -> Result<Value, Error> {
            let Rest(args) = args;
            let template = state.env().get_template(&macro_template)?;
            let macro_state = template.eval_to_state(())?;
            let value = macro_state.lookup(&macro_name).ok_or_else(|| {
                Error::new(
                    ErrorKind::InvalidOperation,
                    format!("macro '{macro_name}' is not defined in template '{macro_template}'"),
                )
            })?;
            value.call(&macro_state, &args)
        },
    );

    Ok(())
}

fn register_manifest_macros(doc: &YamlValue, env: &mut Environment) -> Result<()> {
    let Some(items) = doc.get("macros") else {
        return Ok(());
    };

    let seq = items
        .as_sequence()
        .ok_or_else(|| anyhow::anyhow!("macros must be a sequence"))?;
    for (idx, item) in seq.iter().enumerate() {
        let mapping = item
            .as_mapping()
            .ok_or_else(|| anyhow::anyhow!("macros[{idx}] must be a mapping"))?;
        let signature_key = YamlValue::String("signature".into());
        let body_key = YamlValue::String("body".into());
        let signature = mapping
            .get(&signature_key)
            .and_then(YamlValue::as_str)
            .ok_or_else(|| anyhow::anyhow!("macros[{idx}] signature must be a string"))?;
        let body = mapping
            .get(&body_key)
            .and_then(YamlValue::as_str)
            .ok_or_else(|| anyhow::anyhow!("macros[{idx}] body must be a string"))?;

        register_macro(env, signature, body, idx)
            .with_context(|| format!("register macro '{signature}'"))?;
    }
    Ok(())
}

/// Parse a manifest string using Jinja for value templating.
///
/// The input YAML must be valid on its own. Jinja expressions are evaluated
/// only inside recognised string fields and the `foreach` and `when` keys.
///
/// # Errors
///
/// Returns an error if YAML parsing or Jinja evaluation fails.
fn from_str_named(yaml: &str, name: &str) -> Result<NetsukeManifest> {
    let mut doc: YamlValue = serde_yml::from_str(yaml).map_err(|e| ManifestError::Parse {
        source: map_yaml_error(e, yaml, name),
    })?;

    let mut jinja = Environment::new();
    jinja.set_undefined_behavior(UndefinedBehavior::Strict);
    // Expose custom helpers to templates.
    jinja.add_function("env", |name: String| env_var(&name));
    jinja.add_function("glob", |pattern: String| glob_paths(&pattern));
    let _stdlib_state = crate::stdlib::register(&mut jinja);

    if let Some(vars) = doc.get("vars").and_then(|v| v.as_mapping()).cloned() {
        for (k, v) in vars {
            let key = k
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("non-string key in vars mapping: {k:?}"))?
                .to_string();
            jinja.add_global(key, Value::from_serialize(v));
        }
    }

    register_manifest_macros(&doc, &mut jinja)?;

    expand_foreach(&mut doc, &jinja)?;

    let manifest: NetsukeManifest =
        serde_yml::from_value(doc).map_err(|e| ManifestError::Parse {
            source: map_yaml_error(e, yaml, name),
        })?;

    render_manifest(manifest, &jinja)
}

/// Parse a manifest string using Jinja for value templating.
///
/// The input YAML must be valid on its own. Jinja expressions are evaluated
/// only inside recognised string fields and the `foreach` and `when` keys.
///
/// # Errors
///
/// Returns an error if YAML parsing or Jinja evaluation fails.
pub fn from_str(yaml: &str) -> Result<NetsukeManifest> {
    from_str_named(yaml, "Netsukefile")
}

/// Load a [`NetsukeManifest`] from the given file path.
///
/// # Errors
///
/// Returns an error if the file cannot be read or the YAML fails to parse.
pub fn from_path(path: impl AsRef<Path>) -> Result<NetsukeManifest> {
    let path_ref = path.as_ref();
    let data = fs::read_to_string(path_ref)
        .with_context(|| format!("failed to read {}", path_ref.display()))?;
    from_str_named(&data, &path_ref.display().to_string())
}

#[cfg(test)]
mod tests;
