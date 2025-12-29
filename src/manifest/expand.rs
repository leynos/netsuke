//! Expands manifest foreach directives into concrete targets.
use super::{ManifestMap, ManifestValue};
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
            if !when_allows(&mut clone, env, &item, index)? {
                continue;
            }
            inject_iteration_vars(&mut clone, &item, index)?;
            items.push(ManifestValue::Object(clone));
        }
        Ok(items)
    } else {
        // For targets without foreach, still evaluate and remove the `when` clause.
        // Use empty context since there's no iteration variable.
        if !when_allows_static(&mut map, env)? {
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
        .context("foreach expression did not yield an iterable")?;
    Ok(iter.collect())
}

/// Evaluate a `when` clause and return whether the target should be included.
///
/// The `when` clause can be either:
/// - A Jinja expression (e.g., `item > 1`) - evaluated via `compile_expression`
/// - A Jinja template (e.g., `{{ path is dir }}`) - evaluated via `render_str`
///
/// Template syntax (containing `{{`) is rendered and checked for truthy output
/// ("true" or "1"). Expression syntax is compiled and evaluated directly.
fn eval_when(env: &Environment, expr: &str, ctx: Value) -> Result<bool> {
    if expr.contains("{{") {
        let rendered = env
            .render_str(expr, ctx)
            .with_context(|| format!("when template evaluation error for '{expr}'"))?;
        // Treat "true" or "1" as truthy, anything else (including "false", "") as falsy
        Ok(matches!(
            rendered.trim().to_lowercase().as_str(),
            "true" | "1"
        ))
    } else {
        let result = eval_expression(env, "when", expr, ctx)?;
        Ok(result.is_true())
    }
}

/// Evaluate a `when` clause for a foreach iteration.
///
/// Injects `item` and `index` into the evaluation context for use in the
/// when expression.
fn when_allows(
    map: &mut ManifestMap,
    env: &Environment,
    item: &Value,
    index: usize,
) -> Result<bool> {
    if let Some(when_val) = map.remove("when") {
        let expr = as_str(&when_val, "when")?;
        eval_when(env, expr, context! { item, index })
    } else {
        Ok(true)
    }
}

/// Evaluate a `when` clause for a non-foreach target.
///
/// Uses an empty context since there is no iteration variable available.
fn when_allows_static(map: &mut ManifestMap, env: &Environment) -> Result<bool> {
    if let Some(when_val) = map.remove("when") {
        let expr = as_str(&when_val, "when")?;
        eval_when(env, expr, context! {})
    } else {
        Ok(true)
    }
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
                        "target.vars must be an object, got: {other:?}"
                    ));
                }
            }
        }
    };

    let vars = vars_value
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("vars entry ensured to be an object"))?;
    vars.insert(
        "item".into(),
        serde_json::to_value(item).context("serialise item")?,
    );
    let index_value = ManifestValue::Number(JsonNumber::from(index as u64));
    vars.insert("index".into(), index_value);
    Ok(())
}

fn as_str<'a>(value: &'a ManifestValue, field: &str) -> Result<&'a str> {
    value
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("{field} must be a string expression"))
}

fn eval_expression(env: &Environment, name: &str, expr: &str, ctx: Value) -> Result<Value> {
    env.compile_expression(expr)
        .with_context(|| format!("{name} expression parse error"))?
        .eval(ctx)
        .with_context(|| format!("{name} evaluation error"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use minijinja::Environment;

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
}
