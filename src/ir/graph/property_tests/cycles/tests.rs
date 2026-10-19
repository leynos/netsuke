//! Unit tests for the cycle property suite.
//!
//! These pin the *generator* rather than the obligation: every shape the
//! property draws must build the dependency relation it names, and the long
//! shape must never be truncated into the short one. The obligation itself is
//! `cycles_are_rejected_at_larger_n`, which lives beside the strategy in
//! `super`.
//!
//! The non-vacuity tally belongs here because it is a claim about a whole run,
//! not about one case. A property whose domain never reaches the long class
//! would pass while exercising nothing Kani cannot already reach.

use super::*;
use proptest::strategy::ValueTree as _;
use std::cell::RefCell;

/// The generated domain reaches every class the obligation names.
///
/// Driven through `TestRunner::run` rather than as a property because the
/// claim is about the run as a whole, not about any one case.
#[test]
fn the_run_reaches_the_required_cycle_classes() {
    let reached: RefCell<Reached> = RefCell::new(Reached::default());
    let mut runner = proptest::test_runner::TestRunner::default();
    runner
        .run(&arb_cycle_case(), |case| {
            // Two statements, not one: a single assignment would hold the
            // mutable borrow while the right-hand side takes a shared one,
            // which `RefCell` rejects at runtime rather than in the type
            // system.
            let next = reached.borrow().extended(case.shape);
            *reached.borrow_mut() = next;
            Ok(())
        })
        .expect("the cycle strategy should be generatable");

    let observed = reached.into_inner();
    assert!(
        observed.self_edges > 0,
        "no self-edge case was generated: {observed:?}"
    );
    assert!(
        observed.short_chains > 0,
        "no short-chain case was generated: {observed:?}"
    );
    assert!(
        observed.long_chains > 0,
        "no long-cycle case was generated, so the class Kani cannot reach is untested: \
         {observed:?}"
    );
    assert!(
        observed.missing_dependencies > 0,
        "no unresolved-dependency case was generated: {observed:?}"
    );
}

/// A closed chain of the requested length is a cycle, and one shorter is
/// the boundary the long class sits above.
#[test]
fn a_closed_chain_has_the_requested_length() {
    let mut deps = vec![Vec::new(); LONG_CYCLE_MIN];
    close_chain(&mut deps, LONG_CYCLE_MIN);
    assert_eq!(
        deps.first().map(Vec::as_slice),
        Some([LONG_CYCLE_MIN - 1].as_slice())
    );
    assert_eq!(
        deps.last().map(Vec::as_slice),
        Some([LONG_CYCLE_MIN - 2].as_slice())
    );
}

/// A long-cycle manifest is rejected.
#[test]
fn a_long_cycle_is_rejected() {
    let case = build_case(Shape::Long, &vec![Vec::new(); LONG_CYCLE_MIN]);
    let result = BuildGraph::from_manifest_for_shell(&case.manifest, SHELL);
    assert!(
        matches!(result, Err(IrGenError::CircularDependency { .. })),
        "expected CircularDependency, got {result:?}"
    );
}

/// The long arm always draws enough targets to carry its full chain.
#[test]
fn a_long_case_is_never_truncated_to_a_short_chain() {
    for _ in 0..CASES {
        let case = arb_cycle_case()
            .new_tree(&mut proptest::test_runner::TestRunner::default())
            .expect("the cycle strategy should be generatable")
            .current();
        if case.shape.is_long() {
            assert!(
                case.manifest.targets.len() >= LONG_CYCLE_MIN,
                "a long case drew only {} targets",
                case.manifest.targets.len(),
            );
        }
    }
}

/// An acyclic manifest with an unresolved dependency lowers.
#[test]
fn an_unresolved_dependency_is_not_a_cycle() {
    let case = build_case(Shape::Acyclic, &vec![Vec::new(); 3]);
    BuildGraph::from_manifest_for_shell(&case.manifest, SHELL)
        .expect("an acyclic manifest must lower");
}
