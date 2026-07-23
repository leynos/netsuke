//! Property tests for IR cycle detection.
//!
//! Exercises `canonicalize_cycle` normalization and `CycleDetector` graph
//! traversal, including cycle detection, stack cleanup, and determinism.

use proptest::prelude::*;
use std::collections::HashMap;

use super::{BuildEdge, CycleDetector, analyse, canonicalize_cycle, canonicalize_cycle_by};

#[path = "cycle_analyse_tests.rs"]
mod analyse_tests;

#[path = "cycle_issue322_property_tests.rs"]
mod issue322_property_tests;

fn path(name: &str) -> camino::Utf8PathBuf {
    camino::Utf8PathBuf::from(name)
}

struct EdgeBuilder {
    output: camino::Utf8PathBuf,
    inputs: Vec<camino::Utf8PathBuf>,
    implicit_deps: Vec<camino::Utf8PathBuf>,
}

impl EdgeBuilder {
    fn new(output: camino::Utf8PathBuf) -> Self {
        Self {
            output,
            inputs: Vec::new(),
            implicit_deps: Vec::new(),
        }
    }

    fn input(mut self, node: camino::Utf8PathBuf) -> Self {
        self.inputs.push(node);
        self
    }

    fn implicit_dep(mut self, node: camino::Utf8PathBuf) -> Self {
        self.implicit_deps.push(node);
        self
    }

    fn build(self) -> BuildEdge {
        BuildEdge {
            action_id: "id".into(),
            inputs: self.inputs,
            implicit_deps: self.implicit_deps,
            explicit_outputs: vec![self.output],
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            phony: false,
            always: false,
        }
    }
}

/// Generate a non-empty list of distinct single-character node names.
fn node_names(min: usize, max: usize) -> impl Strategy<Value = Vec<camino::Utf8PathBuf>> {
    proptest::collection::vec("[a-z]", min..=max)
        .prop_filter("nodes must be unique", |v| {
            let set: std::collections::HashSet<_> = v.iter().collect();
            set.len() == v.len()
        })
        .prop_map(|v| v.iter().map(|s| path(s)).collect())
}

/// Build a closed cycle from `nodes`: [...nodes, nodes[0]].
fn make_cycle(nodes: &[camino::Utf8PathBuf]) -> Vec<camino::Utf8PathBuf> {
    let mut cycle = nodes.to_vec();
    if let Some(first) = nodes.first() {
        cycle.push(first.clone());
    }
    cycle
}

/// Assert that `canonicalize_cycle` transforms `input` into `expected`.
///
/// Converts the provided `Utf8PathBuf` slices to an owned `Vec`, calls
/// `canonicalize_cycle`, and asserts the result equals `expected` to verify
/// correct canonicalisation of cycle rotation.
fn check_canonicalize_cycle(input: &[camino::Utf8PathBuf], expected: &[camino::Utf8PathBuf]) {
    let canonical = canonicalize_cycle(input.to_vec());
    assert_eq!(canonical, expected);
}

/// Build a target graph containing two disjoint two-node cycles:
/// p ↔ q and x ↔ y.
fn two_disjoint_cycles() -> HashMap<camino::Utf8PathBuf, BuildEdge> {
    let mut targets = HashMap::new();
    targets.insert(
        path("p"),
        EdgeBuilder::new(path("p")).input(path("q")).build(),
    );
    targets.insert(
        path("q"),
        EdgeBuilder::new(path("q")).input(path("p")).build(),
    );
    targets.insert(
        path("x"),
        EdgeBuilder::new(path("x")).input(path("y")).build(),
    );
    targets.insert(
        path("y"),
        EdgeBuilder::new(path("y")).input(path("x")).build(),
    );
    targets
}

proptest! {
    /// Canonicalization is idempotent: applying it twice yields the same
    /// result as applying it once.
    #[test]
    fn canonicalize_is_idempotent(nodes in node_names(2, 10)) {
        let cycle = make_cycle(&nodes);
        let once = canonicalize_cycle(cycle.clone());
        let twice = canonicalize_cycle(once.clone());
        prop_assert_eq!(once, twice);
    }

    /// All rotations of a cycle canonicalize to the same sequence.
    #[test]
    fn all_rotations_canonicalize_identically(nodes in node_names(2, 8)) {
        let base = canonicalize_cycle(make_cycle(&nodes));
        for i in 1..nodes.len() {
            let mut rotated = nodes.clone();
            rotated.rotate_left(i);
            let result = canonicalize_cycle(make_cycle(&rotated));
            prop_assert_eq!(&base, &result);
        }
    }

    /// The first node in the canonical form is lexicographically <= every
    /// other non-terminal node.
    #[test]
    fn canonical_first_node_is_smallest(nodes in node_names(2, 10)) {
        let canonical = canonicalize_cycle(make_cycle(&nodes));
        let Some(interior) = canonical.get(..canonical.len().saturating_sub(1)) else {
            prop_assert!(false, "canonicalize_cycle produces a valid cycle slice");
            return Ok(());
        };
        let Some(first) = canonical.first() else {
            prop_assert!(false, "canonicalize_cycle produces at least one node");
            return Ok(());
        };
        for node in interior {
            prop_assert!(first <= node);
        }
    }

    /// The canonical form is closed: first and last elements are equal.
    #[test]
    fn canonical_cycle_is_closed(nodes in node_names(2, 10)) {
        let canonical = canonicalize_cycle(make_cycle(&nodes));
        prop_assert_eq!(canonical.first(), canonical.last());
    }
}

#[test]
fn find_cycle_is_deterministic() -> Result<(), String> {
    let targets = two_disjoint_cycles();

    let Some(first) = CycleDetector::find_cycle(&targets) else {
        return Err("expected an initial cycle".into());
    };
    for _ in 1..100 {
        let Some(cycle) = CycleDetector::find_cycle(&targets) else {
            return Err("expected a cycle on every deterministic run".into());
        };
        if cycle != first {
            return Err(format!(
                "find_cycle returned inconsistent results across runs: \
                 first={first:?}, got={cycle:?}",
            ));
        }
    }
    // Run 100 times; `detect` sorts keys deterministically, so the result must
    // be identical on every invocation.
    Ok(())
}

#[test]
fn canonicalize_cycle_rotates_smallest_node() {
    check_canonicalize_cycle(
        &[path("c"), path("a"), path("b"), path("c")],
        &[path("a"), path("b"), path("c"), path("a")],
    );
}

#[test]
fn canonicalize_cycle_handles_reverse_direction() {
    check_canonicalize_cycle(
        &[path("c"), path("b"), path("a"), path("c")],
        &[path("a"), path("c"), path("b"), path("a")],
    );
}

#[test]
fn canonicalize_cycle_by_rotates_smallest_node() {
    assert_eq!(
        canonicalize_cycle_by(vec![2_u8, 0, 1, 2], std::cmp::Ord::cmp),
        vec![0, 1, 2, 0],
    );
}

#[test]
fn canonicalize_cycle_by_preserves_cycle_orientation() {
    assert_eq!(
        canonicalize_cycle_by(vec![2_u8, 1, 0, 2], std::cmp::Ord::cmp),
        vec![0, 2, 1, 0],
    );
}

#[test]
fn find_cycle_detects_one_of_multiple_disjoint_cycles() -> Result<(), String> {
    let targets = two_disjoint_cycles();

    let Some(cycle) = CycleDetector::find_cycle(&targets) else {
        return Err("should detect a cycle in a graph with two disjoint cycles".into());
    };
    // Must be exactly one of the two canonical closed cycles.
    let expected_cycles = vec![
        vec![path("p"), path("q"), path("p")],
        vec![path("x"), path("y"), path("x")],
    ];
    if !expected_cycles.contains(&cycle) {
        return Err(format!(
            "Expected one of {expected_cycles:?}, got {cycle:?}"
        ));
    }
    Ok(())
}

/// Repeated detection on the same detector resets stale traversal state.
#[test]
fn cycle_detector_repeated_detect_resets_traversal_state() {
    let mut targets = HashMap::new();
    targets.insert(
        path("a"),
        EdgeBuilder::new(path("a")).input(path("b")).build(),
    );
    targets.insert(
        path("b"),
        EdgeBuilder::new(path("b")).input(path("a")).build(),
    );

    let expected = vec![path("a"), path("b"), path("a")];
    let mut detector = CycleDetector::new(&targets);

    assert_eq!(detector.detect(), Some(expected.clone()));
    assert_eq!(detector.detect(), Some(expected));
}

/// Generate a list of `count` distinct node names "n0", "n1", …
fn sequential_nodes(count: usize) -> Vec<camino::Utf8PathBuf> {
    (0..count).map(|i| path(&format!("n{i}"))).collect()
}

/// Build an acyclic chain: n0 → n1 → … → n(count-1) (no back-edges).
fn make_acyclic_chain(
    nodes: &[camino::Utf8PathBuf],
) -> HashMap<camino::Utf8PathBuf, super::super::BuildEdge> {
    let mut targets = HashMap::new();
    let mut iter = nodes.iter().peekable();
    while let Some(name) = iter.next() {
        let mut builder = EdgeBuilder::new(name.clone());
        if let Some(next) = iter.peek() {
            builder = builder.input((*next).clone());
        }
        targets.insert(name.clone(), builder.build());
    }
    targets
}

/// Build a cycle graph from the provided node names, starting at node index 0.
///
/// Each node depends on the next name in the slice, and the last node depends
/// on the first, preserving the input order as the cycle order.
fn make_cycle_graph(
    nodes: &[camino::Utf8PathBuf],
) -> HashMap<camino::Utf8PathBuf, super::super::BuildEdge> {
    let mut targets = HashMap::new();
    let deps = nodes.iter().cycle().skip(1).take(nodes.len());
    for (name, dep) in nodes.iter().zip(deps) {
        targets.insert(
            name.clone(),
            EdgeBuilder::new(name.clone()).input(dep.clone()).build(),
        );
    }
    targets
}

proptest! {
    /// detect() returns None for acyclic chains and leaves the stack empty.
    #[test]
    fn detect_acyclic_graph_leaves_stack_empty(count in 2usize..=8) {
        let nodes = sequential_nodes(count);
        let targets = make_acyclic_chain(&nodes);
        let mut detector = CycleDetector::new(&targets);
        prop_assert!(detector.detect().is_none());
        prop_assert!(
            detector.stack.is_empty(),
            "stack must be empty after acyclic traversal",
        );
    }

    /// detect() returns Some for cyclic graphs and leaves the stack empty.
    #[test]
    fn detect_cyclic_graph_leaves_stack_empty(count in 2usize..=8) {
        let nodes = sequential_nodes(count);
        let targets = make_cycle_graph(&nodes);
        let mut detector = CycleDetector::new(&targets);
        prop_assert!(detector.detect().is_some());
        prop_assert!(
            detector.stack.is_empty(),
            "stack must be empty after cycle detection",
        );
    }

    /// detect() is deterministic: all invocations on the same graph return
    /// the same canonical cycle.
    #[test]
    fn detect_is_deterministic_on_cyclic_graphs(count in 2usize..=6) {
        let nodes = sequential_nodes(count);
        let targets = make_cycle_graph(&nodes);
        let first = CycleDetector::find_cycle(&targets);
        for _ in 1..20 {
            let next = CycleDetector::find_cycle(&targets);
            prop_assert_eq!(&first, &next);
        }
    }

    /// detect() reset semantics hold across cyclic graphs of several sizes.
    #[test]
    fn repeated_detect_resets_state_for_cyclic_graphs(count in 2usize..=8) {
        let nodes = sequential_nodes(count);
        let targets = make_cycle_graph(&nodes);
        let expected = CycleDetector::find_cycle(&targets);
        let mut detector = CycleDetector::new(&targets);

        prop_assert_eq!(detector.detect(), expected.clone());
        prop_assert_eq!(detector.detect(), expected);
        prop_assert!(
            detector.stack.is_empty(),
            "stack must be empty after repeated detection",
        );
    }
}
