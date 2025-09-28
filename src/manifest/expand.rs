//! Expands manifest foreach directives into concrete targets.
use anyhow::{Context, Result};
use minijinja::{Environment, context, value::Value};
use serde_yml::{Mapping as YamlMapping, Value as YamlValue};

pub(crate) fn expand_foreach(doc: &mut YamlValue, env: &Environment) -> Result<()> {
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
            return Err(anyhow::anyhow!(
                "target.vars must be a mapping, got: {other:?}"
            ));
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
        let mut doc: YamlValue = serde_yml::from_str(
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
            .and_then(|v| v.as_sequence())
            .expect("targets sequence");
        assert_eq!(targets.len(), 2);
        for (idx, target) in targets.iter().enumerate() {
            let map = target.as_mapping().expect("target map");
            let vars_key = YamlValue::String("vars".into());
            let vars = map
                .get(&vars_key)
                .and_then(|v| v.as_mapping())
                .expect("vars map");
            let index_key = YamlValue::String("index".into());
            let index_val = vars.get(&index_key).expect("index value");
            let item_key = YamlValue::String("item".into());
            let item_val = vars.get(&item_key).expect("item value");
            let YamlValue::Number(index_num) = index_val else {
                panic!("index should be numeric: {index_val:?}");
            };
            assert_eq!(index_num.as_u64().expect("u64"), idx as u64);
            let YamlValue::Number(item_num) = item_val else {
                panic!("item should be numeric: {item_val:?}");
            };
            assert_eq!(item_num.as_u64().expect("u64"), (idx + 1) as u64);
        }
        Ok(())
    }

    #[test]
    fn expand_foreach_applies_when_expression() -> Result<()> {
        let env = Environment::new();
        let mut doc: YamlValue = serde_yml::from_str(
            "targets:
  - name: literal
    foreach: '[1, 2, 3]'
    when: 'item > 1'",
        )?;
        expand_foreach(&mut doc, &env)?;
        let targets = doc
            .get("targets")
            .and_then(|v| v.as_sequence())
            .expect("targets sequence");
        assert_eq!(targets.len(), 2);
        let indexes: Vec<u64> = targets
            .iter()
            .map(|target| {
                let map = target.as_mapping().expect("target map");
                let vars_key = YamlValue::String("vars".into());
                let vars = map
                    .get(&vars_key)
                    .and_then(|v| v.as_mapping())
                    .expect("vars map");
                let index_key = YamlValue::String("index".into());
                let YamlValue::Number(num) = vars.get(&index_key).expect("index value") else {
                    panic!("index missing");
                };
                num.as_u64().expect("u64")
            })
            .collect();
        assert_eq!(indexes, vec![1, 2]);
        Ok(())
    }
}
