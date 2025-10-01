use indexmap::IndexMap;
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
    groups: IndexMap<String, Vec<Value>>,
}

impl GroupedValues {
    fn new(groups: IndexMap<String, Vec<Value>>) -> Self {
        Self { groups }
    }
}

impl Object for GroupedValues {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Map
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        let keys: Vec<Value> = self.groups.keys().cloned().map(Value::from).collect();
        Enumerator::Values(keys)
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        let name = key.as_str()?;
        self.groups
            .get(name)
            .map(|items| Value::from_serialize(items.clone()))
    }
}

fn uniq_filter(values: &Value) -> Result<Value, Error> {
    let iter = values.try_iter()?;
    let mut uniques = Vec::new();

    for item in iter {
        if uniques.iter().all(|existing| existing != &item) {
            uniques.push(item);
        }
    }

    Ok(Value::from_serialize(uniques))
}

fn flatten_filter(values: &Value) -> Result<Value, Error> {
    let iter = values.try_iter()?;
    let mut flattened = Vec::new();

    for item in iter {
        match item.kind() {
            ValueKind::Seq | ValueKind::Iterable => {
                let nested = item.try_iter()?;
                flattened.extend(nested);
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

fn group_by_filter(values: &Value, attr: &str) -> Result<Value, Error> {
    if attr.trim().is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "group_by requires a non-empty attribute".to_string(),
        ));
    }

    let mut groups: IndexMap<String, Vec<Value>> = IndexMap::new();
    let iter = values.try_iter()?;

    for item in iter {
        let key_value = resolve_group_key(&item, attr)?;
        let fallback = key_value.to_string();
        let key = key_value
            .to_str()
            .map_or_else(|| fallback, |s| s.to_string());
        groups.entry(key).or_default().push(item);
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
    use minijinja::context;
    use rstest::rstest;

    fn render_filter(template: &str, ctx: Value) -> Result<Value, Error> {
        let mut env = Environment::new();
        register_filters(&mut env);
        env.compile_expression(template)?.eval(ctx)
    }

    #[rstest]
    fn uniq_filter_removes_duplicates() {
        let ctx = context! { values => vec![1, 1, 2, 2, 3] };
        let result = render_filter("values | uniq", ctx).expect("uniq result");
        let iter = result.try_iter().expect("iter");
        let collected: Vec<_> = iter.map(|value| value.to_string()).collect();
        assert_eq!(collected, vec!["1", "2", "3"]);
    }

    #[rstest]
    fn flatten_filter_joins_nested_sequences() {
        let ctx = context! { values => vec![vec![1, 2], vec![3], Vec::<u8>::new()] };
        let result = render_filter("values | flatten", ctx).expect("flatten result");
        let iter = result.try_iter().expect("iter");
        let collected: Vec<_> = iter.map(|value| value.to_string()).collect();
        assert_eq!(collected, vec!["1", "2", "3"]);
    }

    #[rstest]
    fn group_by_filter_clusters_items() {
        #[derive(serde::Serialize)]
        struct Item {
            class: &'static str,
            value: u8,
        }

        let ctx = context! { values => vec![
            Item { class: "a", value: 1 },
            Item { class: "a", value: 2 },
            Item { class: "b", value: 3 },
        ]};
        let result = render_filter("values | group_by('class')", ctx).expect("group_by result");
        let group_a = result
            .get_attr("a")
            .expect("group a")
            .try_iter()
            .expect("iter")
            .count();
        let group_b = result
            .get_attr("b")
            .expect("group b")
            .try_iter()
            .expect("iter")
            .count();
        assert_eq!(group_a, 2);
        assert_eq!(group_b, 1);
    }
}
