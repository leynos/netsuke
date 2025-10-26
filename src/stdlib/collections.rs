//! Collection filters available in the `MiniJinja` standard library.

use indexmap::{IndexMap, IndexSet};
use std::sync::Arc;

use minijinja::{
    Environment, Error, ErrorKind,
    value::{Enumerator, Object, ObjectRepr, Value, ValueKind},
};

pub(crate) fn register_filters(env: &mut Environment<'_>) {
    env.add_filter("uniq", |values: Value| uniq_filter(&values));
    env.add_filter("flatten", |values: Value| flatten_filter(&values));
    env.add_filter("group_by", |values: Value, attr: String| {
        group_by_filter(&values, &attr)
    });
}

#[derive(Debug)]
struct GroupedValues {
    groups: IndexMap<Value, Vec<Value>>,
    string_keys: IndexMap<String, Value>,
}

impl GroupedValues {
    fn new(groups: IndexMap<Value, Vec<Value>>) -> Self {
        let mut string_keys = IndexMap::new();
        for key in groups.keys() {
            if let Some(label) = key.as_str() {
                string_keys
                    .entry(label.to_owned())
                    .or_insert_with(|| key.clone());
            }
        }
        Self {
            groups,
            string_keys,
        }
    }
}

impl Object for GroupedValues {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Map
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        let keys: Vec<Value> = self.groups.keys().cloned().collect();
        Enumerator::Values(keys)
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        if let Some(name) = key.as_str()
            && let Some(actual_key) = self.string_keys.get(name)
        {
            return self
                .groups
                .get(actual_key)
                .map(|items| Value::from_serialize(items.clone()));
        }

        self.groups
            .get(key)
            .map(|items| Value::from_serialize(items.clone()))
    }
}

fn uniq_filter(values: &Value) -> Result<Value, Error> {
    let iter = values.try_iter()?;
    let mut uniques: IndexSet<Value> = IndexSet::new();

    for item in iter {
        uniques.insert(item);
    }

    let items: Vec<_> = uniques.into_iter().collect();
    Ok(Value::from_serialize(items))
}

fn flatten_filter(values: &Value) -> Result<Value, Error> {
    let iter = values.try_iter()?;
    let mut flattened = Vec::new();

    for item in iter {
        match item.kind() {
            ValueKind::Seq | ValueKind::Iterable => {
                collect_flattened_values(item, &mut flattened)?;
            }
            kind => {
                return Err(Error::new(
                    ErrorKind::InvalidOperation,
                    format!("flatten expected sequence items but found {kind}"),
                ));
            }
        }
    }

    Ok(Value::from_serialize(flattened))
}

fn collect_flattened_values(value: Value, output: &mut Vec<Value>) -> Result<(), Error> {
    match value.kind() {
        ValueKind::Seq | ValueKind::Iterable => {
            for nested in value.try_iter()? {
                collect_flattened_values(nested, output)?;
            }
            Ok(())
        }
        _ => {
            output.push(value);
            Ok(())
        }
    }
}

fn group_by_filter(values: &Value, attr: &str) -> Result<Value, Error> {
    if attr.trim().is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "group_by requires a non-empty attribute".to_owned(),
        ));
    }

    let mut groups: IndexMap<Value, Vec<Value>> = IndexMap::new();
    let iter = values.try_iter()?;

    for item in iter {
        let key_value = resolve_group_key(&item, attr)?;
        groups.entry(key_value.clone()).or_default().push(item);
    }

    Ok(Value::from_object(GroupedValues::new(groups)))
}

fn resolve_group_key(item: &Value, attr: &str) -> Result<Value, Error> {
    match item.get_attr(attr) {
        Ok(value) => ensure_resolved(value, attr, item),
        Err(err) if err.kind() == ErrorKind::InvalidOperation => {
            let key = item.get_item(&Value::from(attr))?;
            ensure_resolved(key, attr, item)
        }
        Err(err) => Err(err),
    }
}

fn ensure_resolved(value: Value, attr: &str, item: &Value) -> Result<Value, Error> {
    if value.is_undefined() {
        Err(Error::new(
            ErrorKind::InvalidOperation,
            format!(
                "group_by could not resolve '{attr}' on item of kind {}",
                item.kind()
            ),
        ))
    } else {
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Result, anyhow, ensure};
    use minijinja::context;
    use rstest::rstest;

    fn render_filter(template: &str, ctx: Value) -> Result<Value, Error> {
        let mut env = Environment::new();
        register_filters(&mut env);
        env.compile_expression(template)?.eval(ctx)
    }

    #[rstest]
    fn uniq_filter_removes_duplicates() -> Result<()> {
        let ctx = context! { values => vec![1, 1, 2, 2, 3, 1] };
        let result = render_filter("values | uniq", ctx)?;
        let iter = result.try_iter()?;
        let collected: Vec<_> = iter.map(|value| format!("{value}")).collect();
        ensure!(
            collected == ["1", "2", "3"],
            "uniq filter output {collected:?} did not match expectation"
        );
        Ok(())
    }

    #[rstest]
    fn flatten_filter_flattens_deeply_nested_sequences() -> Result<()> {
        let ctx = context! { values => vec![vec![vec![1], vec![2]], vec![vec![3]]] };
        let result = render_filter("values | flatten", ctx)?;
        let iter = result.try_iter()?;
        let collected: Vec<_> = iter.map(|value| format!("{value}")).collect();
        ensure!(
            collected == ["1", "2", "3"],
            "flatten filter output {collected:?} did not match expectation"
        );
        Ok(())
    }

    #[rstest]
    fn flatten_filter_rejects_scalars() -> Result<()> {
        let ctx = context! { values => vec!["a", "b"] };
        match render_filter("values | flatten", ctx) {
            Ok(value) => Err(anyhow!(
                "flatten should error for scalars but returned {value:?}"
            )),
            Err(err) => {
                ensure!(
                    err.kind() == ErrorKind::InvalidOperation,
                    "unexpected error kind {kind:?}",
                    kind = err.kind()
                );
                Ok(())
            }
        }
    }

    #[rstest]
    fn group_by_filter_clusters_items_and_preserves_key_types() -> Result<()> {
        #[derive(serde::Serialize)]
        struct Item {
            kind: Value,
            value: &'static str,
        }

        let ctx = context! { values => vec![
            Item { kind: Value::from(1), value: "one" },
            Item { kind: Value::from(1), value: "two" },
            Item { kind: Value::from("label"), value: "three" },
        ]};
        let result = render_filter("values | group_by('kind')", ctx)?;

        let numeric_iter = result.get_item(&Value::from(1))?.try_iter()?;
        let numeric_group = numeric_iter.count();
        let labelled_iter = result.get_attr("label")?.try_iter()?;
        let labelled_group = labelled_iter.count();

        ensure!(
            numeric_group == 2,
            "expected numeric group length 2 but observed {numeric_group}"
        );
        ensure!(
            labelled_group == 1,
            "expected labelled group length 1 but observed {labelled_group}"
        );
        Ok(())
    }

    #[rstest]
    fn group_by_filter_rejects_empty_attribute() -> Result<()> {
        let ctx = context! { values => vec![context!(kind => "tool")] };
        match render_filter("values | group_by('')", ctx) {
            Ok(value) => Err(anyhow!(
                "group_by should reject empty attribute but returned {value:?}"
            )),
            Err(err) => {
                ensure!(
                    err.kind() == ErrorKind::InvalidOperation,
                    "unexpected error kind {kind:?}",
                    kind = err.kind()
                );
                Ok(())
            }
        }
    }
}
