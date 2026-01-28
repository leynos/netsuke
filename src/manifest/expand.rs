//! Expands manifest foreach directives into concrete targets.
use super::{ManifestMap, ManifestValue};
use crate::localization::{self, keys};
use anyhow::{Context, Result};
use minijinja::{Environment, context, value::Value};
use serde_json::{Number as JsonNumber, map::Entry};

/// Expand manifest targets defined with the `foreach` key.
///
/// # Errors
///
/// Returns an error when evaluating `foreach` or `when` expressions, when
/// iteration values fail to serialise, or when target metadata is malformed.
pub fn expand_foreach(doc: &mut ManifestValue, env: &Environment) -> Result<()> {
    let Some(targets) = doc.get_mut("targets").and_then(|v| v.as_array_mut()) else {
        return Ok(());
    };

    let mut expanded = Vec::new();
    for target in std::mem::take(targets) {
        match target {
            ManifestValue::Object(map) => expanded.extend(expand_target(map, env)?),
            other => expanded.push(other),
        }
    }

    *targets = expanded;
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
mod tests {
    use super::*;
    use minijinja::Environment;
    use rstest::rstest;

    fn targets(doc: &ManifestValue) -> Result<&[ManifestValue]> {
        doc.get("targets")
            .and_then(|v| v.as_array())
            .map(Vec::as_slice)
            .context("targets sequence missing")
    }

    #[test]
    fn expand_foreach_expands_sequence_values() -> Result<()> {
        let env = Environment::new();
        let mut doc: ManifestValue = serde_saphyr::from_str(
            "targets:
  - name: literal
    foreach:
      - 1
      - 2
    vars:
      static: keep",
        )?;
        expand_foreach(&mut doc, &env)?;
        let targets = targets(&doc)?;
        anyhow::ensure!(targets.len() == 2, "expected two targets");
        for (idx, target) in targets.iter().enumerate() {
            let map = target.as_object().context("target map")?;
            let vars = map
                .get("vars")
                .and_then(|v| v.as_object())
                .context("vars map")?;
            let index_val = vars.get("index").context("index value")?;
            let item_val = vars.get("item").context("item value")?;
            let ManifestValue::Number(index_num) = index_val else {
                anyhow::bail!("index should be numeric: {index_val:?}");
            };
            let index = index_num
                .as_u64()
                .context("numeric index conversion failed")?;
            anyhow::ensure!(index == idx as u64, "unexpected index value: {index}");
            let ManifestValue::Number(item_num) = item_val else {
                anyhow::bail!("item should be numeric: {item_val:?}");
            };
            let item = item_num
                .as_u64()
                .context("numeric item conversion failed")?;
            anyhow::ensure!(item == (idx + 1) as u64, "unexpected item value: {item}");
        }
        Ok(())
    }

    #[test]
    fn expand_foreach_applies_when_expression() -> Result<()> {
        let env = Environment::new();
        let mut doc: ManifestValue = serde_saphyr::from_str(
            "targets:
  - name: literal
    foreach: '[1, 2, 3]'
    when: 'item > 1'",
        )?;
        expand_foreach(&mut doc, &env)?;
        let targets = targets(&doc)?;
        anyhow::ensure!(targets.len() == 2, "expected filtered targets");
        let indexes: Vec<u64> = targets
            .iter()
            .map(|target| -> Result<u64> {
                let map = target.as_object().context("target map")?;
                let vars = map
                    .get("vars")
                    .and_then(|v| v.as_object())
                    .context("vars map")?;
                let index_value = vars.get("index").context("index value")?;
                let ManifestValue::Number(num) = index_value else {
                    anyhow::bail!("index missing");
                };
                num.as_u64().context("numeric index conversion failed")
            })
            .collect::<Result<_>>()?;
        anyhow::ensure!(
            indexes == vec![1, 2],
            "unexpected filtered indexes: {:?}",
            indexes
        );
        Ok(())
    }

    #[test]
    fn expand_foreach_preserves_object_key_order() -> Result<()> {
        let env = Environment::new();
        let yaml = r"targets:
  - name: literal
    vars:
      existing: keep
    foreach:
      - 1
      - 2
    when: 'true'
    after: done
";
        let mut doc: ManifestValue = serde_saphyr::from_str(yaml)?;
        expand_foreach(&mut doc, &env)?;
        let targets = targets(&doc)?;
        anyhow::ensure!(targets.len() == 2, "expected expanded targets");
        for target in targets {
            let map = target.as_object().context("target object")?;
            let keys: Vec<&str> = map.keys().map(String::as_str).collect();
            anyhow::ensure!(
                keys == ["name", "vars", "after"],
                "key order should remain stable: {:?}",
                keys
            );
        }
        Ok(())
    }

    #[rstest]
    #[case("false", 0, "expression false drops target")]
    #[case("0", 0, "expression 0 drops target")]
    #[case("true", 1, "expression true keeps target")]
    #[case("1 == 1", 1, "expression equality keeps target")]
    #[case("{{ 0 }}", 0, "template 0 drops target")]
    #[case("{{ 1 }}", 1, "template 1 keeps target")]
    #[case("{{ \"true\" }}", 1, "template lowercase true keeps target")]
    #[case("{{ \"True\" }}", 1, "template mixed case True keeps target")]
    #[case("{{ \"TRUE\" }}", 1, "template uppercase TRUE keeps target")]
    #[case("{{ 2 }}", 0, "template 2 drops target (only 1 is truthy)")]
    #[case("{{ \"yes\" }}", 0, "template yes drops target (only true/1 truthy)")]
    fn expand_static_target_when_evaluation(
        #[case] when_expr: &str,
        #[case] expected_count: usize,
        #[case] description: &str,
    ) -> Result<()> {
        let env = Environment::new();
        let yaml = format!("targets:\n  - name: target\n    when: '{when_expr}'");
        let mut doc: ManifestValue = serde_saphyr::from_str(&yaml)?;
        expand_foreach(&mut doc, &env)?;
        let targets = targets(&doc)?;
        anyhow::ensure!(
            targets.len() == expected_count,
            "{description}: expected {expected_count} target(s), got {}",
            targets.len()
        );
        if expected_count == 1 {
            let target = targets.first().context("target")?;
            let map = target.as_object().context("target object")?;
            anyhow::ensure!(
                !map.contains_key("when"),
                "{description}: when field should be removed after evaluation"
            );
        }
        Ok(())
    }

    #[rstest]
    #[case("{{ unclosed", "malformed template")]
    #[case("", "empty when expression")]
    fn expand_static_target_when_invalid_errors(
        #[case] when_expr: &str,
        #[case] description: &str,
    ) -> Result<()> {
        let env = Environment::new();
        let yaml = format!("targets:\n  - name: target\n    when: '{when_expr}'");
        let mut doc: ManifestValue = serde_saphyr::from_str(&yaml)?;
        let result = expand_foreach(&mut doc, &env);
        anyhow::ensure!(result.is_err(), "{description} should return Err");
        Ok(())
    }
}
