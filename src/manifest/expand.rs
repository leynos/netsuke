//! Expands manifest foreach directives into concrete targets and actions.
use super::{ManifestMap, ManifestValue};
use crate::localization::{self, keys};
use anyhow::{Context, Result};
use minijinja::{Environment, context, value::Value};
use serde_json::{Number as JsonNumber, map::Entry};

/// Expand manifest targets and actions defined with the `foreach` key.
///
/// # Errors
///
/// Returns an error when evaluating `foreach` or `when` expressions, when
/// iteration values fail to serialise, or when target metadata is malformed.
pub fn expand_foreach(doc: &mut ManifestValue, env: &Environment) -> Result<()> {
    expand_section(doc, "targets", env)?;
    expand_section(doc, "actions", env)
}

fn expand_section(doc: &mut ManifestValue, key: &str, env: &Environment) -> Result<()> {
    let Some(entries) = doc.get_mut(key).and_then(|v| v.as_array_mut()) else {
        return Ok(());
    };

    let mut expanded = Vec::new();
    for entry in std::mem::take(entries) {
        match entry {
            ManifestValue::Object(map) => expanded.extend(expand_target(map, env)?),
            other => expanded.push(other),
        }
    }

    *entries = expanded;
    Ok(())
}

fn expand_target(mut map: ManifestMap, env: &Environment) -> Result<Vec<ManifestValue>> {
    if let Some(expr_val) = map.get("foreach") {
        let values = parse_foreach_values(expr_val, env)?;
        let mut items = Vec::new();
        for (index, item) in values.into_iter().enumerate() {
            let mut clone = map.clone();
            clone.remove("foreach");
            if !when_allows(&mut clone, env, Some((&item, index)))? {
                continue;
            }
            inject_iteration_vars(&mut clone, &item, index)?;
            items.push(ManifestValue::Object(clone));
        }
        Ok(items)
    } else {
        // For targets without foreach, still evaluate and remove the `when` clause.
        // Use empty context since there's no iteration variable.
        if !when_allows(&mut map, env, None)? {
            return Ok(vec![]);
        }
        Ok(vec![ManifestValue::Object(map)])
    }
}

fn parse_foreach_values(expr_val: &ManifestValue, env: &Environment) -> Result<Vec<Value>> {
    if let Some(seq) = expr_val.as_array() {
        return Ok(seq.iter().cloned().map(Value::from_serialize).collect());
    }
    let expr = as_str(expr_val, "foreach")?;
    let seq = eval_expression(env, "foreach", expr, context! {})?;
    let iter = seq
        .try_iter()
        .context(localization::message(keys::MANIFEST_FOREACH_NOT_ITERABLE))?;
    Ok(iter.collect())
}

/// Evaluate a `when` clause and return whether the target should be included.
///
/// The `when` clause can be either:
/// - A Jinja expression (e.g., `item > 1`) - evaluated via `compile_expression`
/// - A Jinja template (e.g., `{{ path is dir }}`) - evaluated via `render_str`
///
/// Detection strategy: attempt expression compilation first; if parsing fails,
/// fall back to template rendering. This avoids brittle heuristics like
/// checking for `{{` which could appear in string literals.
///
/// Empty expressions are rejected as invalid.
fn eval_when(env: &Environment, expr: &str, ctx: Value) -> Result<bool> {
    anyhow::ensure!(
        !expr.is_empty(),
        "{}",
        localization::message(keys::MANIFEST_WHEN_EMPTY)
    );

    // Try expression compilation first - this handles plain expressions
    // like "item > 1" or "true" without needing template delimiters.
    if let Ok(compiled) = env.compile_expression(expr) {
        let result = compiled.eval(ctx).with_context(|| {
            localization::message(keys::MANIFEST_WHEN_EVAL_ERROR).with_arg("expr", expr)
        })?;
        return Ok(result.is_true());
    }

    // Expression parsing failed - treat as template syntax (e.g., "{{ path is dir }}")
    let rendered = env.render_str(expr, ctx).with_context(|| {
        localization::message(keys::MANIFEST_WHEN_TEMPLATE_ERROR).with_arg("expr", expr)
    })?;
    // Treat "true" or "1" as truthy, anything else (including "false", "") as falsy
    Ok(matches!(
        rendered.trim().to_lowercase().as_str(),
        "true" | "1"
    ))
}

/// Evaluate a `when` clause if present, returning whether the target should be included.
///
/// Accepts an optional iteration context (`item`, `index`) for foreach targets;
/// static targets pass `None`.
fn when_allows(
    map: &mut ManifestMap,
    env: &Environment,
    iteration: Option<(&Value, usize)>,
) -> Result<bool> {
    let Some(when_val) = map.remove("when") else {
        return Ok(true);
    };
    let expr = as_str(&when_val, "when")?;
    let ctx = match iteration {
        Some((item, index)) => context! { item, index },
        None => context! {},
    };
    eval_when(env, expr, ctx)
}

fn inject_iteration_vars(map: &mut ManifestMap, item: &Value, index: usize) -> Result<()> {
    let vars_value = match map.entry("vars") {
        Entry::Vacant(slot) => slot.insert(ManifestValue::Object(ManifestMap::new())),
        Entry::Occupied(slot) => {
            let value = slot.into_mut();
            match value {
                ManifestValue::Object(_) => value,
                other => {
                    return Err(anyhow::anyhow!(
                        "{}",
                        localization::message(keys::MANIFEST_TARGET_VARS_NOT_OBJECT)
                            .with_arg("value", format!("{other:?}"))
                    ));
                }
            }
        }
    };

    let vars = vars_value.as_object_mut().ok_or_else(|| {
        anyhow::anyhow!(
            "{}",
            localization::message(keys::MANIFEST_VARS_ENTRY_NOT_OBJECT)
        )
    })?;
    vars.insert(
        "item".into(),
        serde_json::to_value(item)
            .context(localization::message(keys::MANIFEST_FOREACH_SERIALISE_ITEM))?,
    );
    let index_value = ManifestValue::Number(JsonNumber::from(index as u64));
    vars.insert("index".into(), index_value);
    Ok(())
}

fn as_str<'a>(value: &'a ManifestValue, field: &str) -> Result<&'a str> {
    value.as_str().ok_or_else(|| {
        anyhow::anyhow!(
            "{}",
            localization::message(keys::MANIFEST_FIELD_NOT_STRING).with_arg("field", field)
        )
    })
}

fn eval_expression(env: &Environment, name: &str, expr: &str, ctx: Value) -> Result<Value> {
    env.compile_expression(expr)
        .with_context(|| {
            localization::message(keys::MANIFEST_EXPRESSION_PARSE_ERROR).with_arg("name", name)
        })?
        .eval(ctx)
        .with_context(|| {
            localization::message(keys::MANIFEST_EXPRESSION_EVAL_ERROR).with_arg("name", name)
        })
}

#[cfg(test)]
#[path = "expand_tests.rs"]
mod tests;
