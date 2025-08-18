//! Manifest loading helpers.
//!
//! This module parses a `Netsukefile` without relying on a global Jinja
//! preprocessing pass. The YAML is parsed first and Jinja expressions are
//! evaluated only within string values or the `foreach` and `when` keys.

use crate::ast::{NetsukeManifest, Recipe, StringOrList, Target, Vars};
use anyhow::{Context, Result, anyhow};
use miette::{Diagnostic, NamedSource, SourceSpan};
use minijinja::{Environment, UndefinedBehavior, context, value::Value};
use serde_spanned::Spanned;
use serde_yml::{Error as YamlError, Location};
use serde_yml::{Mapping as YamlMapping, Value as YamlValue};
use std::fmt::Write;
use std::{fs, path::Path};
use thiserror::Error;

const ERR_MANIFEST_PARSE: &str = "manifest parse error";

// Compute a narrow highlight span from a location.
fn to_span(src: &str, loc: Location) -> SourceSpan {
    let at = loc.index();
    let end = if src.as_bytes().get(at).is_some_and(|b| *b != b'\n') {
        at + 1
    } else {
        at
    };
    let span = Spanned::new(at..end, ());
    SourceSpan::new(span.span().start.into(), span.span().len().into())
}

#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
struct YamlDiagnostic {
    #[source_code]
    src: NamedSource,
    #[label("{label}")]
    span: Option<SourceSpan>,
    #[help]
    help: Option<String>,
    #[source]
    source: Option<anyhow::Error>,
    #[diagnostic(code(netsuke.yaml.parse))]
    message: String,
    label: String,
}

fn hint_for(err_str: &str, src: &str) -> Option<String> {
    let lower = err_str.to_lowercase();
    if src.contains('\t') {
        Some("Use spaces for indentation; tabs are invalid in YAML.".into())
    } else if lower.contains("did not find expected '-'") {
        Some("Start list items with '-' and ensure proper indentation.".into())
    } else if lower.contains("expected ':'") {
        Some("Ensure each key is followed by ':' separating key and value.".into())
    } else {
        None
    }
}

fn map_yaml_error(err: &YamlError, src: &str) -> anyhow::Error {
    let (line, col, span) = err.location().map_or((1, 1, None), |l| {
        (l.line(), l.column(), Some(to_span(src, l)))
    });
    let err_str = err.to_string();
    let hint = hint_for(&err_str, src);
    let mut message = format!("YAML parse error at line {line}, column {col}: {err_str}");
    if let Some(h) = &hint {
        write!(message, " Hint: {h}").expect("string write");
    }

    let diag = YamlDiagnostic {
        src: NamedSource::new("manifest.yml", src.to_string()),
        span,
        help: hint,
        source: Some(anyhow::Error::msg(err_str.clone())),
        message,
        label: "parse error here".into(),
    };

    anyhow::Error::new(diag)
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
    let mut doc: YamlValue = serde_yml::from_str(yaml).map_err(|e| map_yaml_error(&e, yaml))?;

    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);

    if let Some(vars) = doc.get("vars").and_then(|v| v.as_mapping()).cloned() {
        for (k, v) in vars {
            let key = k
                .as_str()
                .ok_or_else(|| anyhow!("non-string key in 'vars' mapping: {k:?}"))?
                .to_string();
            env.add_global(key, Value::from_serialize(v));
        }
    }

    expand_foreach(&mut doc, &env)?;

    let manifest: NetsukeManifest = serde_yml::from_value(doc).context(ERR_MANIFEST_PARSE)?;

    render_manifest(manifest, &env)
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
        .context("foreach expression did not yield an iterable")?;
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
            return Err(anyhow!("target.vars must be a mapping, got: {other:?}"));
        }
    };
    vars.insert(
        YamlValue::String("item".into()),
        serde_yml::to_value(item).context("serialise item")?,
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
        .with_context(|| format!("{field} must be a string expression"))
}

fn eval_expression(env: &Environment, name: &str, expr: &str, ctx: Value) -> Result<Value> {
    env.compile_expression(expr)
        .with_context(|| format!("{name} expression parse error"))?
        .eval(ctx)
        .with_context(|| format!("{name} evaluation error"))
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
        *desc = env
            .render_str(desc, context! {})
            .context("render rule description")?;
    }
    render_string_or_list(&mut rule.deps, env, &Vars::new())?;
    match &mut rule.recipe {
        Recipe::Command { command } => {
            *command = env
                .render_str(command, context! {})
                .context("render rule command")?;
        }
        Recipe::Script { script } => {
            *script = env
                .render_str(script, context! {})
                .context("render rule script")?;
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
            *command = env
                .render_str(command, &target.vars)
                .context("render target command")?;
        }
        Recipe::Script { script } => {
            *script = env
                .render_str(script, &target.vars)
                .context("render target script")?;
        }
        Recipe::Rule { rule } => render_string_or_list(rule, env, &target.vars)?,
    }
    Ok(())
}

fn render_vars(vars: &mut Vars, env: &Environment) -> Result<()> {
    let snapshot = vars.clone();
    for (key, value) in vars.iter_mut() {
        if let YamlValue::String(s) = value {
            *s = env
                .render_str(s, &snapshot)
                .with_context(|| format!("render var '{key}'"))?;
        }
    }
    Ok(())
}

fn render_string_or_list(value: &mut StringOrList, env: &Environment, ctx: &Vars) -> Result<()> {
    match value {
        StringOrList::String(s) => {
            *s = env.render_str(s, ctx).context("render string value")?;
        }
        StringOrList::List(list) => {
            for item in list {
                *item = env.render_str(item, ctx).context("render list value")?;
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
        .with_context(|| format!("Failed to read {}", path_ref.display()))?;
    from_str(&data)
}
