//! Unit tests for the stdlib collection helpers and the flatten allowance.
//!
//! The allowance cases use the private [`FlattenLimits::for_test`] seam so each
//! ceiling is proven at its exact threshold and one past it. A default-sized
//! graph would only show that *some* limit fires; these prove which limit fires
//! and where.

use super::flatten::{FlattenLimits, flatten_filter_with_limits};
use super::*;
use anyhow::{Result, anyhow, bail, ensure};
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
#[case::uniq_keeps_first_seen_order(
    "values | uniq | join(',')",
    context! { values => vec![1, 1, 2, 2, 3, 1] },
    "1,2,3"
)]
#[case::flatten_is_depth_first(
    "values | flatten | join(',')",
    context! { values => vec![vec![vec![1], vec![2]], vec![vec![3]]] },
    "1,2,3"
)]
fn filters_render_expected_values(
    #[case] expression: &str,
    #[case] ctx: Value,
    #[case] expected: &str,
) -> Result<()> {
    let rendered = render_filter(expression, ctx)?;
    ensure!(
        format!("{rendered}") == expected,
        "{expression} should render {expected:?}, but rendered {rendered:?}"
    );
    Ok(())
}

#[rstest]
#[case::flatten_rejects_scalars("values | flatten", context! { values => vec!["a", "b"] })]
#[case::group_by_rejects_empty_attribute(
    "values | group_by('')",
    context! { values => vec![context!(kind => "tool")] }
)]
fn filters_reject_invalid_input(#[case] expression: &str, #[case] ctx: Value) -> Result<()> {
    match render_filter(expression, ctx) {
        Ok(value) => Err(anyhow!("{expression} should fail but returned {value:?}")),
        Err(err) => {
            ensure!(
                err.kind() == ErrorKind::InvalidOperation,
                "{expression} should report InvalidOperation, but was {:?}",
                err.kind()
            );
            Ok(())
        }
    }
}

/// The outcome one exact-threshold case expects.
#[derive(Clone, Copy, Debug)]
enum Expectation {
    /// The traversal succeeds and retains this many leaves.
    Retains(usize),
    /// The traversal is rejected with this message fragment.
    Rejects(&'static str),
}

#[rstest]
#[case::depth_at_ceiling(
    FlattenLimits::for_test(4, 100, 100),
    Value::from(vec![nested(Value::from(1), 3)]),
    Expectation::Retains(1)
)]
#[case::depth_past_ceiling(
    FlattenLimits::for_test(4, 100, 100),
    Value::from(vec![nested(Value::from(1), 4)]),
    Expectation::Rejects("exceeded the nesting depth limit")
)]
#[case::visits_at_ceiling(
    FlattenLimits::for_test(8, 2, 10),
    Value::from(vec![
        Value::from(Vec::<Value>::new()),
        Value::from(Vec::<Value>::new()),
    ]),
    Expectation::Retains(0)
)]
#[case::visits_past_ceiling(
    FlattenLimits::for_test(8, 1, 10),
    Value::from(vec![
        Value::from(Vec::<Value>::new()),
        Value::from(Vec::<Value>::new()),
    ]),
    Expectation::Rejects("exceeded the traversal visit limit")
)]
#[case::leaves_at_ceiling(
    FlattenLimits::for_test(8, 10, 3),
    Value::from(vec![
        Value::from(vec![Value::from(1), Value::from(2)]),
        Value::from(vec![Value::from(3)]),
    ]),
    Expectation::Retains(3)
)]
#[case::leaves_past_ceiling(
    FlattenLimits::for_test(8, 10, 2),
    Value::from(vec![
        Value::from(vec![Value::from(1), Value::from(2)]),
        Value::from(vec![Value::from(3)]),
    ]),
    Expectation::Rejects("exceeded the retained item limit")
)]
fn flatten_threshold_cases(
    #[case] limits: FlattenLimits,
    #[case] subject: Value,
    #[case] expectation: Expectation,
) -> Result<()> {
    match (flatten_filter_with_limits(&subject, &limits), expectation) {
        (Ok(flattened), Expectation::Retains(expected)) => {
            let retained = flattened.try_iter()?.count();
            ensure!(
                retained == expected,
                "the traversal should retain {expected} leaves, but retained {retained}"
            );
            Ok(())
        }
        (Ok(flattened), Expectation::Rejects(fragment)) => Err(anyhow!(
            "the traversal should be rejected with {fragment:?}, but flattened to {flattened:?}"
        )),
        (Err(err), Expectation::Rejects(fragment)) => ensure_limit_error(&err, fragment),
        (Err(err), Expectation::Retains(expected)) => Err(anyhow!(
            "the traversal should retain {expected} leaves, but failed: {err}"
        )),
    }
}

/// One unbounded graph shape the traversal must reject.
#[derive(Clone, Copy, Debug)]
enum UnboundedGraph {
    /// A shared graph with no leaf at all.
    SharedEmptyList,
    /// An iterable that never ends, with a panic sentinel.
    EndlessIterable,
    /// A lazily self-nesting object.
    SelfNesting,
}

/// Build the subject for one unbounded graph shape.
fn unbounded_graph(graph: UnboundedGraph) -> Value {
    match graph {
        UnboundedGraph::SharedEmptyList => {
            let mut shared = Value::from(Vec::<Value>::new());
            for _ in 0..20 {
                shared = Value::from(vec![shared.clone(), shared.clone()]);
            }
            Value::from(vec![shared])
        }
        UnboundedGraph::EndlessIterable => {
            let endless = Value::make_iterable(|| {
                (0_u32..).map(|index| {
                    assert!(
                        index <= 8,
                        "the traversal must stop at the visit ceiling but read item {index}"
                    );
                    Value::from(index)
                })
            });
            Value::from(vec![endless])
        }
        UnboundedGraph::SelfNesting => {
            let cell: Arc<OnceLock<Value>> = Arc::new(OnceLock::new());
            let nesting = Value::make_object_iterable(Arc::clone(&cell), |stored| {
                let reference = stored
                    .get()
                    .expect("the self reference is stored before traversal")
                    .clone();
                Box::new(std::iter::once(reference))
            });
            cell.set(nesting.clone())
                .expect("the cell is empty before the first store");
            Value::from(vec![nesting])
        }
    }
}

#[rstest]
#[case::shared_empty_list(
    UnboundedGraph::SharedEmptyList,
    FlattenLimits::for_test(64, 32, 8),
    "exceeded the traversal visit limit"
)]
#[case::endless_iterable(
    UnboundedGraph::EndlessIterable,
    FlattenLimits::for_test(8, 4, 16),
    "exceeded the traversal visit limit"
)]
#[case::self_nesting(
    UnboundedGraph::SelfNesting,
    FlattenLimits::for_test(4, 100, 100),
    "exceeded the nesting depth limit"
)]
fn flatten_bounds_unbounded_graphs(
    #[case] graph: UnboundedGraph,
    #[case] limits: FlattenLimits,
    #[case] fragment: &str,
) -> Result<()> {
    let subject = unbounded_graph(graph);
    let Err(err) = flatten_filter_with_limits(&subject, &limits) else {
        bail!("{graph:?} must be rejected with {fragment:?}");
    };
    ensure_limit_error(&err, fragment)
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
