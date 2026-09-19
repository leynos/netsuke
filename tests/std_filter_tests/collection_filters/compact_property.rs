//! The `OBL-COMPACT` property: `compact` is an order-preserving filter with a
//! stable predicate.
//!
//! A table of hand-picked lists could show that the predicate drops the right
//! members one case at a time, but not that the result is always a subsequence
//! of its input, that discarding blanks is idempotent, or that a retained
//! member always survives. Those are statements about the whole domain, so the
//! domain is generated.

use super::fallible;
use anyhow::{Context, Result};
use minijinja::{Environment, context, value::Value};
use proptest::prelude::*;
use proptest::test_runner::{FileFailurePersistence, TestRunner};
use std::cell::Cell;

/// A member drawn from the decision boundary of the blank predicate.
///
/// Every arm is either a member the contract retains or one it drops, and the
/// two groups are weighted evenly so a run meets both. Whitespace is here
/// because a naive `trim().is_empty()` reading of "blank" would drop it, while
/// the contract keeps it; `0` and `false` are here because a naive truthiness
/// reading would drop them, and the contract keeps them.
fn member() -> impl Strategy<Value = Value> {
    prop_oneof![
        // Droppable: none, undefined, the empty string.
        2 => Just(Value::from(())),
        2 => Just(Value::UNDEFINED),
        2 => Just(Value::from("")),
        // Retained, and the two a truthiness reading would wrongly drop.
        2 => Just(Value::from(0)),
        2 => Just(Value::from(false)),
        // Retained, and the one a `trim().is_empty()` reading would drop.
        1 => Just(Value::from(" ")),
        // Retained.
        4 => "[a-z]{1,3}".prop_map(|seed| Value::from(seed.as_str())),
    ]
}

/// Whether the contract drops `value`.
fn is_droppable(value: &Value) -> bool {
    value.is_none() || value.is_undefined() || value.as_str().is_some_and(str::is_empty)
}

/// Extract the members of `value`, which the probe has already made a list.
fn members_of(value: &Value) -> Result<Vec<Value>> {
    let iter = value
        .try_iter()
        .context("the compacted result should be iterable")?;
    Ok(iter.collect())
}

/// Render `{{ values | compact | list }}` under the stdlib environment.
fn compacted(env: &Environment<'_>, values: &[Value]) -> Result<Vec<Value>> {
    let template = env
        .compile_expression("values | compact | list")
        .context("compile the compact probe")?;
    let result = template
        .eval(context!(values => values))
        .context("render the compact probe")?;
    members_of(&result)
}

proptest! {
    // The corpus is cheap — one template render per case — so 128 cases still
    // reach a wide spread of sequence lengths and member mixes.
    #![proptest_config(ProptestConfig {
        cases: 128,
        // Name the file explicitly. The default `SourceParallel` policy looks
        // for a `lib.rs` or `main.rs` beside the source and gives up in an
        // integration-test crate, so recorded seeds were neither written nor
        // replayed — the file on disk was inert.
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/std_filter_tests.proptest-regressions",
        ))),
        ..ProptestConfig::default()
    })]

    /// Every invariant of `OBL-COMPACT` holds at once, over one sequence.
    ///
    /// Checking them together keeps the corpus small: one render serves all
    /// four claims, and a failure names the sequence that produced it.
    #[test]
    fn compact_is_order_preserving_and_idempotent(
        values in prop::collection::vec(member(), 0..8),
        seed in "[a-z]{1,3}",
    ) {
        let env = fallible::stdlib_env()
            .map_err(|error| TestCaseError::fail(error.to_string()))?;

        // A member the caller can recognise inside the output. It is appended
        // rather than generated into the sequence because it proves, case by
        // case, that a retained member survives rendering: without it an
        // implementation returning `[]` for every input would satisfy the
        // remaining claims.
        let mut subject = values.clone();
        subject.push(Value::from(seed.as_str()));

        let observed = compacted(&env, &subject)
            .map_err(|error| TestCaseError::fail(format!("{error:#}")))?;

        // (d) never discards a member the contract retains — see the appended
        // sentinel above.
        prop_assert!(observed.contains(&Value::from(seed.as_str())),
            "a retained member must survive compact: {subject:?} -> {observed:?}");

        // (a) contains no blank member.
        for member in &observed {
            prop_assert!(!is_droppable(member),
                "compact must not emit a blank member: {subject:?} -> {observed:?}");
        }

        // (b) is a subsequence of the input.
        let expected: Vec<Value> = subject
            .iter()
            .filter(|item| !is_droppable(item))
            .cloned()
            .collect();
        prop_assert_eq!(&observed, &expected,
            "compact must drop exactly the blank members, in order");

        // (c) is idempotent.
        let twice = compacted(&env, &observed)
            .map_err(|error| TestCaseError::fail(format!("{error:#}")))?;
        prop_assert_eq!(&twice, &observed,
            "compacting an already-compacted sequence must change nothing");
    }
}

/// The corpus reaches both sides of the drop/retain boundary.
///
/// Without this the property could hold over a corpus that only ever generated
/// droppable members — or only ever generated retained ones — and say nothing
/// about the other half of the predicate. The deterministic witness case covers
/// the boundary as a pair, so this asserts that the *generated* domain does too.
#[test]
fn the_generated_corpus_spans_the_drop_retain_boundary() {
    let mut runner = TestRunner::new(ProptestConfig {
        cases: 128,
        ..ProptestConfig::default()
    });
    // `TestRunner::run` takes an `Fn`, so the tallies live in a `Cell`.
    let tallies: Cell<(usize, usize)> = Cell::new((0, 0));
    runner
        .run(&prop::collection::vec(member(), 0..8), |values| {
            tallies.set(
                values
                    .iter()
                    .fold(tallies.get(), |(droppable, retained), value| {
                        if is_droppable(value) {
                            (droppable + 1, retained)
                        } else {
                            (droppable, retained + 1)
                        }
                    }),
            );
            Ok(())
        })
        .expect("the member corpus should be generatable");
    let (droppable, retained) = tallies.get();
    assert!(
        droppable > 0 && retained > 0,
        "the corpus must contain both droppable and retained members: \
         {droppable} droppable, {retained} retained"
    );
}
