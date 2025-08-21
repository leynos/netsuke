//! Manifest loading helpers.
//!
//! This module parses a `Netsukefile` without relying on a global Jinja
//! preprocessing pass. The YAML is parsed first and Jinja expressions are
//! evaluated only within string values or the `foreach` and `when` keys. It
//! exposes an `env()` function to surface environment variables, failing fast
//! when values are missing or invalid.

use crate::{
    ast::{NetsukeManifest, Recipe, StringOrList, Target, Vars},
    diagnostics::ResultExt,
};
use miette::{Diagnostic, NamedSource, Report, Result, SourceSpan};
use minijinja::{Environment, Error, ErrorKind, UndefinedBehavior, context, value::Value};
use serde_yml::{Error as YamlError, Location};
use serde_yml::{Mapping as YamlMapping, Value as YamlValue};
use std::{fs, path::Path};
use thiserror::Error;
const YAML_HINTS: &[(&str, &str)] = &[
    (
        "did not find expected '-'",
        "Start list items with '-' and ensure proper indentation.",
    ),
    (
        "expected ':'",
        "Ensure each key is followed by ':' separating key and value.",
    ),
    (
        "mapping values are not allowed",
        "Check for a stray ':' or add quotes around values where needed.",
    ),
    (
        "found character that cannot start any token",
        "Remove stray characters and ensure indentation uses spaces (no tabs).",
    ),
    (
        "unknown escape character",
        "Use valid YAML escape sequences or quote the string.",
    ),
];

// Compute a narrow highlight span from a location.
fn to_span(src: &str, loc: Location) -> SourceSpan {
    let at = loc.index();
    let bytes = src.as_bytes();
    let (start, end) = match bytes.get(at) {
        Some(&b) if b != b'\n' => (at, at + 1),
        _ => {
            // Fallback: highlight the previous byte on the same line when possible.
            let start = if at > 0 && bytes.get(at - 1).is_some_and(|p| *p != b'\n') {
                at - 1
            } else {
                at
            };
            (start, at)
        }
    };
    let len = end.saturating_sub(start);
    #[allow(clippy::useless_conversion, reason = "future-proof span length type")]
    SourceSpan::new(start.into(), len.into())
}

#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
#[diagnostic(code(netsuke::yaml::parse))]
pub struct YamlDiagnostic {
    #[source_code]
    src: NamedSource<String>,
    #[label("parse error here")]
    span: Option<SourceSpan>,
    #[help]
    help: Option<String>,
    #[source]
    source: YamlError,
    message: String,
}

fn has_tab_indent(src: &str, loc: Option<Location>) -> bool {
    let Some(loc) = loc else { return false };
    let idx = loc.index();
    let bytes = src.as_bytes();
    let line_start = bytes
        .get(..idx)
        .and_then(|b| b.iter().rposition(|b| *b == b'\n').map(|p| p + 1))
        .unwrap_or(0);
    let line_end = bytes
        .get(idx..)
        .and_then(|b| b.iter().position(|b| *b == b'\n').map(|p| idx + p))
        .unwrap_or(bytes.len());
    bytes
        .get(line_start..line_end)
        .unwrap_or(&[])
        .iter()
        .take_while(|b| **b == b' ' || **b == b'\t')
        .any(|b| *b == b'\t')
}

fn hint_for(err_str: &str, src: &str, loc: Option<Location>) -> Option<String> {
    if has_tab_indent(src, loc) {
        return Some("Use spaces for indentation; tabs are invalid in YAML.".into());
    }
    let lower = err_str.to_lowercase();
    YAML_HINTS
        .iter()
        .find(|(needle, _)| lower.contains(needle))
        .map(|(_, hint)| (*hint).into())
}

#[derive(Debug, Error, Diagnostic)]
pub enum ManifestError {
    #[error("manifest parse error")]
    #[diagnostic(code(netsuke::manifest::parse))]
    Parse {
        #[source]
        #[diagnostic_source]
        source: YamlDiagnostic,
    },
}

fn map_yaml_error(err: YamlError, src: &str, name: &str) -> YamlDiagnostic {
    let loc = err.location();
    let (line, col, span) = loc.map_or((1, 1, None), |l| {
        (l.line(), l.column(), Some(to_span(src, l)))
    });
    let err_str = err.to_string();
    let hint = hint_for(&err_str, src, loc);
    let message = format!("YAML parse error at line {line}, column {col}: {err_str}");

    YamlDiagnostic {
        src: NamedSource::new(name, src.to_string()),
        span,
        help: hint,
        source: err,
        message,
    }
}

/// Resolve the value of an environment variable for the `env()` Jinja helper.
///
/// Returns the variable's value or a structured error that mirrors Jinja's
/// failure modes, ensuring templates halt when a variable is missing or not
/// valid UTF-8.
///
/// # Examples
///
/// ```ignore
/// use netsuke::manifest::env_var;
///
/// std::env::set_var("EXAMPLE_KEY", "value");
/// assert_eq!(env_var("EXAMPLE_KEY").unwrap(), "value");
/// std::env::remove_var("EXAMPLE_KEY");
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

/// Parse a manifest string using Jinja for value templating.
///
/// The input YAML must be valid on its own. Jinja expressions are evaluated
/// only inside recognised string fields and the `foreach` and `when` keys.
///
/// # Errors
///
/// Returns an error if YAML parsing or Jinja evaluation fails.
fn from_str_named(yaml: &str, name: &str) -> Result<NetsukeManifest> {
    let mut doc: YamlValue =
        serde_yml::from_str(yaml).map_err(|e| Report::new(map_yaml_error(e, yaml, name)))?;

    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    // Expose a strict environment variable accessor to templates.
    env.add_function("env", |name: String| env_var(&name));

    if let Some(vars) = doc.get("vars").and_then(|v| v.as_mapping()).cloned() {
        for (k, v) in vars {
            let key = k
                .as_str()
                .ok_or_else(|| Report::msg(format!("non-string key in 'vars' mapping: {k:?}")))?
                .to_string();
            env.add_global(key, Value::from_serialize(v));
        }
    }

    expand_foreach(&mut doc, &env)?;

    let manifest: NetsukeManifest = serde_yml::from_value(doc).map_err(|e| {
        Report::new(ManifestError::Parse {
            source: map_yaml_error(e, yaml, name),
        })
    })?;

    render_manifest(manifest, &env)
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

/// Expand `foreach` entries within the raw YAML document.
fn expand_foreach(doc: &mut YamlValue, env: &Environment) -> Result<()> {
    let Some(targets) = doc.get_mut("targets").and_then(|v| v.as_sequence_mut()) else {
        return Ok(());
    };

    let mut expanded = Vec::new();
    for target in std::mem::take(targets) {
        match target {
            YamlValue::Mapping(map) => expanded.extend(expand_target(map, env)?),
            other => expanded.push(other),
        }
    }

    *targets = expanded;
    Ok(())
}

fn expand_target(map: YamlMapping, env: &Environment) -> Result<Vec<YamlValue>> {
    let foreach_key = YamlValue::String("foreach".into());
    if let Some(expr_val) = map.get(&foreach_key) {
        let values = parse_foreach_values(expr_val, env)?;
        let mut items = Vec::new();
        for (index, item) in values.into_iter().enumerate() {
            let mut clone = map.clone();
            clone.remove(&foreach_key);
            if !when_allows(&mut clone, env, &item, index)? {
                continue;
            }
            inject_iteration_vars(&mut clone, &item, index)?;
            items.push(YamlValue::Mapping(clone));
        }
        Ok(items)
    } else {
        Ok(vec![YamlValue::Mapping(map)])
    }
}

fn parse_foreach_values(expr_val: &YamlValue, env: &Environment) -> Result<Vec<Value>> {
    if let Some(seq) = expr_val.as_sequence() {
        return Ok(seq.iter().cloned().map(Value::from_serialize).collect());
    }
    let expr = as_str(expr_val, "foreach")?;
    let seq = eval_expression(env, "foreach", expr, context! {})?;
    let iter = seq
        .try_iter()
        .diag("foreach expression did not yield an iterable")?;
    Ok(iter.collect())
}

fn when_allows(
    map: &mut YamlMapping,
    env: &Environment,
    item: &Value,
    index: usize,
) -> Result<bool> {
    let when_key = YamlValue::String("when".into());
    if let Some(when_val) = map.remove(&when_key) {
        let expr = as_str(&when_val, "when")?;
        let result = eval_expression(env, "when", expr, context! { item, index })?;
        Ok(result.is_true())
    } else {
        Ok(true)
    }
}

fn inject_iteration_vars(map: &mut YamlMapping, item: &Value, index: usize) -> Result<()> {
    let vars_key = YamlValue::String("vars".into());
    let mut vars = match map.remove(&vars_key) {
        None => YamlMapping::new(),
        Some(YamlValue::Mapping(m)) => m,
        Some(other) => {
            return Err(Report::msg(format!(
                "target.vars must be a mapping, got: {other:?}"
            )));
        }
    };
    vars.insert(
        YamlValue::String("item".into()),
        serde_yml::to_value(item).diag("serialise item")?,
    );
    vars.insert(
        YamlValue::String("index".into()),
        YamlValue::Number(u64::try_from(index).expect("index overflow").into()),
    );
    map.insert(vars_key, YamlValue::Mapping(vars));
    Ok(())
}

fn as_str<'a>(value: &'a YamlValue, field: &str) -> Result<&'a str> {
    value
        .as_str()
        .ok_or_else(|| Report::msg(format!("{field} must be a string expression")))
}

fn eval_expression(env: &Environment, name: &str, expr: &str, ctx: Value) -> Result<Value> {
    env.compile_expression(expr)
        .diag_with(|| format!("{name} expression parse error"))?
        .eval(ctx)
        .diag_with(|| format!("{name} evaluation error"))
}

/// Render a Jinja template and label any error with the given context.
fn render_str_with(
    env: &Environment,
    tpl: &str,
    ctx: &impl serde::Serialize,
    what: impl FnOnce() -> String,
) -> Result<String> {
    env.render_str(tpl, ctx).diag_with(what)
}

/// Render all templated strings in the manifest.
fn render_manifest(mut manifest: NetsukeManifest, env: &Environment) -> Result<NetsukeManifest> {
    for action in &mut manifest.actions {
        render_target(action, env)?;
    }
    for target in &mut manifest.targets {
        render_target(target, env)?;
    }
    for rule in &mut manifest.rules {
        render_rule(rule, env)?;
    }
    Ok(manifest)
}

fn render_rule(rule: &mut crate::ast::Rule, env: &Environment) -> Result<()> {
    if let Some(desc) = &mut rule.description {
        *desc = render_str_with(env, desc, &context! {}, || "render rule description".into())?;
    }
    render_string_or_list(&mut rule.deps, env, &Vars::new())?;
    match &mut rule.recipe {
        Recipe::Command { command } => {
            *command =
                render_str_with(env, command, &context! {}, || "render rule command".into())?;
        }
        Recipe::Script { script } => {
            *script = render_str_with(env, script, &context! {}, || "render rule script".into())?;
        }
        Recipe::Rule { rule: r } => render_string_or_list(r, env, &Vars::new())?,
    }
    Ok(())
}

fn render_target(target: &mut Target, env: &Environment) -> Result<()> {
    render_vars(&mut target.vars, env)?;
    render_string_or_list(&mut target.name, env, &target.vars)?;
    render_string_or_list(&mut target.sources, env, &target.vars)?;
    render_string_or_list(&mut target.deps, env, &target.vars)?;
    render_string_or_list(&mut target.order_only_deps, env, &target.vars)?;
    match &mut target.recipe {
        Recipe::Command { command } => {
            *command = render_str_with(env, command, &target.vars, || {
                "render target command".into()
            })?;
        }
        Recipe::Script { script } => {
            *script = render_str_with(env, script, &target.vars, || "render target script".into())?;
        }
        Recipe::Rule { rule } => render_string_or_list(rule, env, &target.vars)?,
    }
    Ok(())
}

fn render_vars(vars: &mut Vars, env: &Environment) -> Result<()> {
    let snapshot = vars.clone();
    for (key, value) in vars.iter_mut() {
        if let YamlValue::String(s) = value {
            *s = render_str_with(env, s, &snapshot, || format!("render var '{key}'"))?;
        }
    }
    Ok(())
}

fn render_string_or_list(value: &mut StringOrList, env: &Environment, ctx: &Vars) -> Result<()> {
    match value {
        StringOrList::String(s) => {
            *s = render_str_with(env, s, ctx, || "render string value".into())?;
        }
        StringOrList::List(list) => {
            for item in list {
                *item = render_str_with(env, item, ctx, || "render list value".into())?;
            }
        }
        StringOrList::Empty => {}
    }
    Ok(())
}

/// Load a [`NetsukeManifest`] from the given file path.
///
/// # Errors
///
/// Returns an error if the file cannot be read or the YAML fails to parse.
pub fn from_path(path: impl AsRef<Path>) -> Result<NetsukeManifest> {
    let path_ref = path.as_ref();
    let data = fs::read_to_string(path_ref)
        .diag_with(|| format!("failed to read {}", path_ref.display()))?;
    from_str_named(&data, &path_ref.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::Error as _;

    #[test]
    fn yaml_error_without_location_defaults_to_first_line() {
        let err = YamlError::custom("boom");
        let report = Report::new(map_yaml_error(err, "", "test"));
        let msg = report.to_string();
        assert!(msg.contains("line 1, column 1"), "message: {msg}");
    }
}
