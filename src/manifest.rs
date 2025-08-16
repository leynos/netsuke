//! Manifest loading helpers.
//!
//! This module parses a `Netsukefile` without relying on a global Jinja
//! preprocessing pass. The YAML is parsed first and Jinja expressions are
//! evaluated only within string values or the `foreach` and `when` keys.

use crate::ast::{NetsukeManifest, Recipe, StringOrList, Target, Vars};
use anyhow::{Context, Result, anyhow};
use minijinja::{Environment, UndefinedBehavior, context, value::Value};
use serde_yml::Value as YamlValue;
use std::{fs, path::Path};

/// Parse a manifest string using Jinja for value templating.
///
/// The input YAML must be valid on its own. Jinja expressions are evaluated
/// only inside recognised string fields and the `foreach` and `when` keys.
///
/// # Errors
///
/// Returns an error if YAML parsing or Jinja evaluation fails.
pub fn from_str(yaml: &str) -> Result<NetsukeManifest> {
    let mut doc: YamlValue = serde_yml::from_str(yaml).context("initial YAML parse error")?;

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

    let manifest: NetsukeManifest = serde_yml::from_value(doc).context("manifest parse error")?;

    render_manifest(manifest, &env)
}

/// Expand `foreach` entries within the raw YAML document.
fn expand_foreach(doc: &mut YamlValue, env: &Environment) -> Result<()> {
    let Some(targets) = doc.get_mut("targets").and_then(|v| v.as_sequence_mut()) else {
        return Ok(());
    };

    let mut expanded = Vec::new();
    for target in std::mem::take(targets) {
        let YamlValue::Mapping(mut map) = target else {
            expanded.push(target);
            continue;
        };

        let foreach_key = YamlValue::String("foreach".into());
        if let Some(expr_val) = map.remove(&foreach_key) {
            let expr = expr_val
                .as_str()
                .context("foreach must be a string expression")?;
            let expr = env
                .compile_expression(expr)
                .context("foreach expression parse error")?;
            let seq = expr.eval(context! {}).context("foreach evaluation error")?;
            let iter = seq
                .try_iter()
                .context("foreach expression did not yield an iterable")?;

            for (index, item) in iter.enumerate() {
                let mut clone = map.clone();

                let when_key = YamlValue::String("when".into());
                if let Some(when_val) = clone.remove(&when_key) {
                    let when_expr = when_val
                        .as_str()
                        .context("when must be a string expression")?;
                    let when = env
                        .compile_expression(when_expr)
                        .context("when expression parse error")?
                        .eval(context! { item, index })
                        .context("when evaluation error")?;
                    if !when.is_true() {
                        continue;
                    }
                }

                let vars_key = YamlValue::String("vars".into());
                let mut vars = clone
                    .remove(&vars_key)
                    .and_then(|v| match v {
                        YamlValue::Mapping(m) => Some(m),
                        _ => None,
                    })
                    .unwrap_or_default();
                vars.insert(
                    YamlValue::String("item".into()),
                    serde_yml::to_value(&item).context("serialise item")?,
                );
                vars.insert(
                    YamlValue::String("index".into()),
                    YamlValue::Number(u64::try_from(index).expect("index overflow").into()),
                );
                clone.insert(vars_key, YamlValue::Mapping(vars));

                expanded.push(YamlValue::Mapping(clone));
            }
        } else {
            expanded.push(YamlValue::Mapping(map));
        }
    }

    *targets = expanded;
    Ok(())
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
