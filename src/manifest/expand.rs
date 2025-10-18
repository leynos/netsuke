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

fn expand_target(map: ManifestMap, env: &Environment) -> Result<Vec<ManifestValue>> {
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

fn when_allows(
    map: &mut ManifestMap,
    env: &Environment,
    item: &Value,
    index: usize,
) -> Result<bool> {
    if let Some(when_val) = map.remove("when") {
        let expr = as_str(&when_val, "when")?;
        let result = eval_expression(env, "when", expr, context! { item, index })?;
        Ok(result.is_true())
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
        .expect("vars entry ensured to be an object");
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
        let targets = doc
            .get("targets")
            .and_then(|v| v.as_array())
            .expect("targets sequence");
        assert_eq!(targets.len(), 2);
        for (idx, target) in targets.iter().enumerate() {
            let map = target.as_object().expect("target map");
            let vars = map
                .get("vars")
                .and_then(|v| v.as_object())
                .expect("vars map");
            let index_val = vars.get("index").expect("index value");
            let item_val = vars.get("item").expect("item value");
            let ManifestValue::Number(index_num) = index_val else {
                panic!("index should be numeric: {index_val:?}");
            };
            assert_eq!(index_num.as_u64().expect("u64"), idx as u64);
            let ManifestValue::Number(item_num) = item_val else {
                panic!("item should be numeric: {item_val:?}");
            };
            assert_eq!(item_num.as_u64().expect("u64"), (idx + 1) as u64);
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
        let targets = doc
            .get("targets")
            .and_then(|v| v.as_array())
            .expect("targets sequence");
        assert_eq!(targets.len(), 2);
        let indexes: Vec<u64> = targets
            .iter()
            .map(|target| {
                let map = target.as_object().expect("target map");
                let vars = map
                    .get("vars")
                    .and_then(|v| v.as_object())
                    .expect("vars map");
                let ManifestValue::Number(num) = vars.get("index").expect("index value") else {
                    panic!("index missing");
                };
                num.as_u64().expect("u64")
            })
            .collect();
        assert_eq!(indexes, vec![1, 2]);
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
        let targets = doc
            .get("targets")
            .and_then(|v| v.as_array())
            .expect("targets sequence");
        assert_eq!(targets.len(), 2);
        for target in targets {
            let map = target.as_object().expect("target object");
            let keys: Vec<&str> = map.keys().map(String::as_str).collect();
            assert_eq!(
                keys,
                ["name", "vars", "after"],
                "key order should remain stable"
            );
        }
        Ok(())
    }
}
