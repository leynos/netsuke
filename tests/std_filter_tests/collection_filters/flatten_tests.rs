//! Behavioural coverage for the `flatten` filter's traversal allowance.
//!
//! `flatten` expands every occurrence of a nested sequence. `MiniJinja` lists
//! share their descendants, so a compact template graph — such as the
//! twenty-fold doubling below — describes an expansion far larger than the
//! template that built it. The regressions here pin the bounded error each
//! allowance reports, and the compatibility cases pin the flatten semantics
//! that must survive the bound: depth-first left-to-right order, repeated
//! leaves, map leaves, and lazy iterables.
//!
//! The counter-bearing map case is the control for the retained-leaf path:
//! flatten must not enumerate a map to decide it is a leaf, so the test
//! asserts the count is still zero before the result is rendered.

use anyhow::{Context, Result, bail, ensure};
use minijinja::{
    Environment, Error, ErrorKind, context,
    value::{Enumerator, Object, ObjectRepr, Value, ValueKind},
};
use rstest::rstest;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use test_support::fluent::normalize_fluent_isolates;

use super::fallible;

/// One compact graph shape a regression renders.
#[derive(Clone, Copy, Debug)]
enum GraphShape {
    /// Twenty doublings of a seed list, which can be `[]` or `[1]`.
    Doubled(&'static str),
    /// Two hundred single-member nestings around an empty list.
    Chain,
}

/// Build a template that constructs one [`GraphShape`] and flattens it.
///
/// A doubling step stores its value in two positions, so the graph after `n`
/// iterations describes `2^n` leaves while holding only `n` lists: exactly the
/// compact expansion the allowance must bound.
fn graph_template(shape: GraphShape) -> String {
    let (seed, repetitions, step) = match shape {
        GraphShape::Doubled(seed) => (seed, 20, "{% set ns.v = [ns.v, ns.v] %}"),
        GraphShape::Chain => ("[]", 200, "{% set ns.v = [ns.v] %}"),
    };
    format!(
        concat!(
            "{{% set ns = namespace(v={seed}) %}}",
            "{{% for _ in range({repetitions}) %}}{step}{{% endfor %}}",
            "{{{{ [ns.v] | flatten | length }}}}"
        ),
        seed = seed,
        repetitions = repetitions,
        step = step
    )
}

/// Render `template` and return the error it must raise.
fn render_error(env: &Environment<'_>, template: &str) -> Result<Error> {
    match env.render_str(template, context! {}) {
        Ok(output) => bail!("expected a bounded flatten error, but rendered {output:?}"),
        Err(err) => Ok(err),
    }
}

/// Assert that `err` is a localized `InvalidOperation` carrying `fragment`.
fn ensure_invalid_operation(err: &Error, fragment: &str) -> Result<()> {
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "flatten should report InvalidOperation, but was {:?}",
        err.kind()
    );
    let message = normalize_fluent_isolates(&err.to_string());
    ensure!(
        message.contains(fragment),
        "error should contain {fragment:?}: {message}"
    );
    Ok(())
}

/// A zero-leaf shared graph fails on the traversal visit allowance.
///
/// The graph has no leaves at all, so only the visit count grows; an
/// implementation that bounds leaves alone never terminates here.
#[test]
fn flatten_rejects_a_shared_zero_leaf_graph_at_the_visit_limit() -> Result<()> {
    let env = fallible::stdlib_env()?;
    let template = graph_template(GraphShape::Doubled("[]"));
    let err = render_error(&env, &template)?;
    ensure_invalid_operation(&err, "exceeded the traversal visit limit")
}

/// A shared graph with one leaf beneath it fails on the retained-item limit.
///
/// Every doubling below the seed doubles the retained leaves, so the leaf
/// allowance is reached long before the visit allowance.
#[test]
fn flatten_rejects_a_shared_graph_at_the_retained_item_limit() -> Result<()> {
    let env = fallible::stdlib_env()?;
    let template = graph_template(GraphShape::Doubled("[1]"));
    let err = render_error(&env, &template)?;
    ensure_invalid_operation(&err, "exceeded the retained item limit")
}

/// A linearly nested chain fails on the nesting-depth allowance.
#[test]
fn flatten_rejects_a_deeply_nested_graph_at_the_depth_limit() -> Result<()> {
    let env = fallible::stdlib_env()?;
    let template = graph_template(GraphShape::Chain);
    let err = render_error(&env, &template)?;
    ensure_invalid_operation(&err, "exceeded the nesting depth limit")
}

/// Finite literal cases keep their documented flatten semantics.
#[rstest]
#[case::empty_lists("[[], []] | flatten | length", "0")]
#[case::single_member_lists("[[1, 2]] | flatten | join(',')", "1,2")]
#[case::depth_first_order("[[1, [2]], [3]] | flatten | join(',')", "1,2,3")]
fn flatten_preserves_supported_semantics(
    #[case] expression: &str,
    #[case] expected: &str,
) -> Result<()> {
    let env = fallible::stdlib_env()?;
    let template = format!("{{{{ {expression} }}}}");
    let output = env
        .render_str(&template, context! {})
        .with_context(|| format!("render {template}"))?;
    ensure!(
        output == expected,
        "{expression} should render {expected:?}, but rendered {output:?}"
    );
    Ok(())
}

/// Immediate scalar members are still rejected, not silently dropped.
#[test]
fn flatten_still_rejects_an_immediate_scalar_member() -> Result<()> {
    let env = fallible::stdlib_env()?;
    let err = render_error(&env, "{{ [[1], 2] | flatten }}")?;
    ensure_invalid_operation(&err, "Flatten expected sequence items")
}

/// Repeating one shared value yields its leaves once per occurrence.
#[test]
fn flatten_repeats_a_shared_reference_without_deduplicating() -> Result<()> {
    let env = fallible::stdlib_env()?;
    let shared = Value::from(vec![Value::from(1), Value::from(2)]);
    let values = vec![shared.clone(), shared];
    let output = env
        .render_str(
            "{{ values | flatten | join(',') }}",
            context!(values => values),
        )
        .context("render a repeated shared reference")?;
    ensure!(
        output == "1,2,1,2",
        "a shared reference must expand once per occurrence, but rendered {output:?}"
    );
    Ok(())
}

/// A map member is a leaf, retained alongside string leaves in order.
#[test]
fn flatten_retains_map_leaves_in_order() -> Result<()> {
    let env = fallible::stdlib_env()?;
    let map = Value::from_iter(vec![(String::from("k"), Value::from(1))]);
    let values = vec![
        Value::from(vec![Value::from("a"), map.clone()]),
        Value::from(vec![Value::from("b")]),
    ];
    let flattened = env
        .compile_expression("values | flatten")
        .context("compile the map-leaf probe")?
        .eval(context!(values => values))
        .context("evaluate the map-leaf probe")?;
    let leaves: Vec<Value> = flattened
        .try_iter()
        .context("the flattened result should be iterable")?
        .collect();
    ensure!(
        leaves.len() == 3,
        "the map and the two strings are three leaves, but got {leaves:?}"
    );
    ensure!(
        leaves.iter().map(Value::kind).collect::<Vec<_>>()
            == [ValueKind::String, ValueKind::Map, ValueKind::String],
        "leaves must keep their kinds in depth-first order, but got {:?}",
        leaves.iter().map(Value::kind).collect::<Vec<_>>()
    );
    Ok(())
}

/// A lazy iterable member is descended into like a rendered list.
#[test]
fn flatten_descends_into_a_lazy_iterable_member() -> Result<()> {
    let env = fallible::stdlib_env()?;
    let lazy = Value::make_iterable(|| (1_u32..3_u32).map(Value::from));
    let values = vec![Value::from(vec![Value::from(0)]), lazy];
    let output = env
        .render_str(
            "{{ values | flatten | join(',') }}",
            context!(values => values),
        )
        .context("render a lazy iterable member")?;
    ensure!(
        output == "0,1,2",
        "a lazy iterable member must be flattened in order, but rendered {output:?}"
    );
    Ok(())
}

/// A lazy iterable root is iterated, and its scalar members are rejected.
#[test]
fn flatten_rejects_a_lazy_root_with_scalar_members() -> Result<()> {
    let env = fallible::stdlib_env()?;
    let lazy = Value::make_iterable(|| (0_u32..2_u32).map(Value::from));
    let expression = env
        .compile_expression("subject | flatten")
        .context("compile the lazy-root probe")?;
    let Err(err) = expression.eval(context!(subject => lazy)) else {
        bail!("a lazy root of scalars must be rejected as immediate member kinds");
    };
    ensure_invalid_operation(&err, "Flatten expected sequence items")
}

/// A map leaf is not enumerated until the result is rendered.
///
/// `flatten` decides membership by `ValueKind`; a `Map` is a leaf. An
/// implementation that probed the object by enumerating it would make the
/// template pay for map traversal before the flattened list is even used, so
/// the count is checked both before and after the result is forced.
#[test]
fn flatten_does_not_enumerate_a_map_leaf() -> Result<()> {
    let env = fallible::stdlib_env()?;
    let enumerations = Arc::new(AtomicUsize::new(0));
    let map = Value::from_object(CountingMap {
        keys: vec![Value::from("k")],
        enumerations: Arc::clone(&enumerations),
    });
    let values = vec![Value::from(vec![map])];

    // The result is discarded deliberately: evaluating the filter is the
    // observation, and the counter below is what the assertion reads.
    env.compile_expression("values | flatten")
        .context("compile the map-leaf laziness probe")?
        .eval(context!(values => values.clone()))
        .context("evaluate the map-leaf laziness probe")?;
    ensure!(
        enumerations.load(Ordering::SeqCst) == 0,
        "flatten must not enumerate a map leaf, but enumerated it {} times",
        enumerations.load(Ordering::SeqCst)
    );

    let rendered = env
        .render_str(
            "{{ values | flatten | join(',') }}",
            context!(values => values),
        )
        .context("render the map leaf")?;
    ensure!(
        enumerations.load(Ordering::SeqCst) > 0,
        "rendering the retained map should enumerate it, but the count stayed zero"
    );
    ensure!(
        rendered.contains('k'),
        "the retained map should render its key, but rendered {rendered:?}"
    );
    Ok(())
}

/// A map object that counts how often a template enumerates it.
#[derive(Debug)]
struct CountingMap {
    /// Keys the map reports when enumerated.
    keys: Vec<Value>,
    /// Number of enumerations observed so far.
    enumerations: Arc<AtomicUsize>,
}

impl Object for CountingMap {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Map
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        self.enumerations.fetch_add(1, Ordering::SeqCst);
        Enumerator::Values(self.keys.clone())
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        self.keys
            .iter()
            .position(|candidate| candidate == key)
            .map(|index| Value::from(index + 1))
    }
}
