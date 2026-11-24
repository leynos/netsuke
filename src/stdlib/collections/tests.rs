//! Unit tests for the stdlib collection helpers and the flatten allowance.
//!
//! The allowance cases use the private [`FlattenLimits::for_test`] seam so each
//! ceiling is proven at its exact threshold and one past it. A default-sized
//! graph would only show that *some* limit fires; these prove which limit fires
//! and where.

use super::flatten::{FlattenLimits, flatten_filter_with_limits};
use super::*;
use anyhow::{Result, anyhow, ensure};
use minijinja::context;
use rstest::rstest;
use std::sync::{Arc, OnceLock};

/// Render one filter expression against a freshly registered environment.
fn render_filter(template: &str, ctx: Value) -> Result<Value, Error> {
    let mut env = Environment::new();
    register_filters(&mut env);
    env.compile_expression(template)?.eval(ctx)
}

/// Assert that `err` is an `InvalidOperation` containing `fragment`.
fn ensure_limit_error(err: &Error, fragment: &str) -> Result<()> {
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "the allowance should report InvalidOperation, but was {:?}",
        err.kind()
    );
    ensure!(
        err.to_string().contains(fragment),
        "the allowance should report {fragment:?}: {err}"
    );
    Ok(())
}

/// Wrap `leaf` in `levels` single-member lists.
fn nested(leaf: Value, levels: usize) -> Value {
    (0..levels).fold(leaf, |inner, _| Value::from(vec![inner]))
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

/// An item exactly at the depth ceiling is allowed.
#[test]
fn flatten_accepts_an_item_at_the_depth_ceiling() -> Result<()> {
    let limits = FlattenLimits::for_test(4, 100, 100);
    let subject = Value::from(vec![nested(Value::from(1), 3)]);
    let flattened = flatten_filter_with_limits(&subject, &limits)?;
    let leaves: Vec<Value> = flattened.try_iter()?.collect();
    ensure!(
        leaves == [Value::from(1)],
        "a graph inside the depth ceiling should flatten, but yielded {leaves:?}"
    );
    Ok(())
}

/// One level past the depth ceiling is rejected.
#[test]
fn flatten_rejects_an_item_past_the_depth_ceiling() -> Result<()> {
    let limits = FlattenLimits::for_test(4, 100, 100);
    let subject = Value::from(vec![nested(Value::from(1), 4)]);
    let err = flatten_filter_with_limits(&subject, &limits)
        .expect_err("a graph past the depth ceiling must be rejected");
    ensure_limit_error(&err, "exceeded the nesting depth limit")
}

/// Empty containers count as visits, so the exact visit count is their number.
#[test]
fn flatten_accepts_the_exact_visit_ceiling() -> Result<()> {
    let limits = FlattenLimits::for_test(8, 2, 10);
    let subject = Value::from(vec![
        Value::from(Vec::<Value>::new()),
        Value::from(Vec::<Value>::new()),
    ]);
    let flattened = flatten_filter_with_limits(&subject, &limits)?;
    ensure!(
        flattened.try_iter()?.next().is_none(),
        "two empty containers retain no leaves, but the result was not empty"
    );
    Ok(())
}

/// One visit past the ceiling is rejected, even for an empty container.
#[test]
fn flatten_rejects_the_visit_after_the_ceiling() -> Result<()> {
    let limits = FlattenLimits::for_test(8, 1, 10);
    let subject = Value::from(vec![
        Value::from(Vec::<Value>::new()),
        Value::from(Vec::<Value>::new()),
    ]);
    let err = flatten_filter_with_limits(&subject, &limits)
        .expect_err("a second empty container must exceed the visit ceiling");
    ensure_limit_error(&err, "exceeded the traversal visit limit")
}

/// Retaining exactly the leaf ceiling succeeds.
#[test]
fn flatten_accepts_the_exact_retained_item_ceiling() -> Result<()> {
    let limits = FlattenLimits::for_test(8, 10, 3);
    let subject = Value::from(vec![
        Value::from(vec![Value::from(1), Value::from(2)]),
        Value::from(vec![Value::from(3)]),
    ]);
    let flattened = flatten_filter_with_limits(&subject, &limits)?;
    ensure!(
        flattened.try_iter()?.count() == 3,
        "three leaves should be retained under a ceiling of three"
    );
    Ok(())
}

/// One retained leaf past the ceiling is rejected.
#[test]
fn flatten_rejects_the_leaf_after_the_ceiling() -> Result<()> {
    let limits = FlattenLimits::for_test(8, 10, 2);
    let subject = Value::from(vec![
        Value::from(vec![Value::from(1), Value::from(2)]),
        Value::from(vec![Value::from(3)]),
    ]);
    let err = flatten_filter_with_limits(&subject, &limits)
        .expect_err("a third leaf must exceed the retained-item ceiling");
    ensure_limit_error(&err, "exceeded the retained item limit")
}

/// A shared empty-list graph is bounded by counting, not by identity.
#[test]
fn flatten_bounds_a_shared_empty_list_graph() -> Result<()> {
    let mut graph = Value::from(Vec::<Value>::new());
    for _ in 0..20 {
        graph = Value::from(vec![graph.clone(), graph.clone()]);
    }
    let subject = Value::from(vec![graph]);
    let limits = FlattenLimits::for_test(64, 32, 8);
    let err = flatten_filter_with_limits(&subject, &limits)
        .expect_err("a shared graph must stop at the visit ceiling");
    ensure_limit_error(&err, "exceeded the traversal visit limit")
}

/// An endless iterable stops at the visit ceiling, before reading much past it.
#[test]
fn flatten_bounds_an_endless_iterable() -> Result<()> {
    let endless = Value::make_iterable(|| {
        (0_u32..).map(|index| {
            assert!(
                index <= 8,
                "the traversal must stop at the visit ceiling but read item {index}"
            );
            Value::from(index)
        })
    });
    let subject = Value::from(vec![endless]);
    let limits = FlattenLimits::for_test(8, 4, 16);
    let err = flatten_filter_with_limits(&subject, &limits)
        .expect_err("an endless iterable must stop at the visit ceiling");
    ensure_limit_error(&err, "exceeded the traversal visit limit")
}

/// A lazily self-nesting object stops at the depth ceiling.
#[test]
fn flatten_bounds_a_self_nesting_lazy_object() -> Result<()> {
    let cell: Arc<OnceLock<Value>> = Arc::new(OnceLock::new());
    let nesting = Value::make_object_iterable(Arc::clone(&cell), |cell| {
        let reference = cell
            .get()
            .expect("the self reference is stored before traversal")
            .clone();
        Box::new(std::iter::once(reference))
    });
    cell.set(nesting.clone())
        .expect("the cell is empty before the first store");
    let subject = Value::from(vec![nesting]);
    let limits = FlattenLimits::for_test(4, 100, 100);
    let err = flatten_filter_with_limits(&subject, &limits)
        .expect_err("a self-nesting object must stop at the depth ceiling");
    ensure_limit_error(&err, "exceeded the nesting depth limit")
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
