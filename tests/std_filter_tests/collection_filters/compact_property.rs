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
use minijinja::{
    Environment, context,
    value::{Value, ValueKind},
};
use proptest::prelude::*;
use proptest::test_runner::{FileFailurePersistence, TestRunner};
use std::cell::Cell;

/// A member drawn from the decision boundary of the blank predicate.
///
/// Every arm is either a member the contract retains or one it drops, and the
/// two groups are weighted evenly so a run meets both. Whitespace is here
/// because a naive `trim().is_empty()` reading of "blank" would drop it, while
/// the contract keeps it; `0` and `false` are here because a naive truthiness
/// reading would drop them, and the contract keeps them; the empty byte array
/// is here because `Value::as_str` answers for well-formed UTF-8 *bytes* too,
/// so a predicate that asked it without checking `ValueKind::String` would drop
/// a byte array on the strength of a text rule.
///
/// The two empty containers are the remaining shape a truthiness reading gets
/// wrong. They are held as distinct kinds rather than as one "empty" arm
/// because the filter's own documentation names both `[]` and `{}`, and a
/// predicate that special-cased one would still pass a corpus that only ever
/// generated the other.
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
        // Retained, and the one an unguarded `as_str()` would wrongly drop:
        // `Value::from_bytes(vec![])` is `ValueKind::Bytes`, not `String`, and
        // its kind is what decides.
        1 => Just(Value::from_bytes(Vec::new())),
        // Retained, and the two a truthiness reading would drop alongside `0`.
        // Distinct arms so the corpus meets each kind on its own.
        1 => Just(empty_list()),
        1 => Just(empty_map()),
        // Retained.
        4 => "[a-z]{1,3}".prop_map(|seed| Value::from(seed.as_str())),
    ]
}

/// Build an empty `Value` of kind [`ValueKind::Seq`].
fn empty_list() -> Value {
    Value::from(Vec::<Value>::new())
}

/// Build an empty `Value` of kind [`ValueKind::Map`].
fn empty_map() -> Value {
    Value::from_iter(Vec::<(String, Value)>::new())
}

/// Whether the contract drops `value`.
///
/// The empty-string arm tests [`ValueKind::String`] rather than asking
/// [`Value::as_str`], matching `stdlib::collections::is_blank`. That method
/// answers for well-formed UTF-8 *bytes* too, so the unguarded form would call
/// `Value::from_bytes(vec![])` an empty string and drop it — a byte array
/// discarded on the strength of a text rule that does not apply to it. The
/// corpus below reaches that value, so this oracle is checked against the
/// production predicate rather than merely restating it.
fn is_droppable(value: &Value) -> bool {
    value.is_none()
        || value.is_undefined()
        || (value.kind() == ValueKind::String && value.as_str().is_some_and(str::is_empty))
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

        // A member the caller can recognize inside the output. It is appended
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
///
/// The two empty containers get their own tally rather than being folded into
/// "retained". They are the arms whose *kind* the property is asserting, so a
/// corpus that happened never to draw one would leave the containment claim
/// untested while the retained total still looked healthy.
#[test]
fn the_generated_corpus_spans_the_drop_retain_boundary() {
    let mut runner = TestRunner::new(ProptestConfig {
        cases: 128,
        ..ProptestConfig::default()
    });
    // `TestRunner::run` takes an `Fn`, so the tallies live in a `Cell`. The
    // tuple is `(droppable, retained, empty seq, empty map)`.
    let tallies: Cell<(usize, usize, usize, usize)> = Cell::new((0, 0, 0, 0));
    runner
        .run(&prop::collection::vec(member(), 0..8), |values| {
            tallies.set(values.iter().fold(
                tallies.get(),
                |(droppable, retained, seqs, maps), value| {
                    let is_empty_sequence = value.kind() == ValueKind::Seq
                        && value.try_iter().is_ok_and(|mut m| m.next().is_none());
                    let is_empty_map = value.kind() == ValueKind::Map
                        && value.try_iter().is_ok_and(|mut m| m.next().is_none());
                    let counts = (usize::from(is_empty_sequence), usize::from(is_empty_map));
                    if is_droppable(value) {
                        (droppable + 1, retained, seqs + counts.0, maps + counts.1)
                    } else {
                        (droppable, retained + 1, seqs + counts.0, maps + counts.1)
                    }
                },
            ));
            Ok(())
        })
        .expect("the member corpus should be generatable");
    let (droppable, retained, empty_seqs, empty_maps) = tallies.get();
    assert!(
        droppable > 0 && retained > 0,
        "the corpus must contain both droppable and retained members: \
         {droppable} droppable, {retained} retained"
    );
    assert!(
        empty_seqs > 0 && empty_maps > 0,
        "the corpus must reach both empty containers, or the property says nothing about \
         them: {empty_seqs} empty sequences, {empty_maps} empty maps"
    );
}
