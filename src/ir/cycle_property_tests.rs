//! Property tests for IR cycle detection.
//!
//! Exercises `canonicalize_cycle` normalization and `CycleDetector` graph
//! traversal, including cycle detection, stack cleanup, and determinism.

use proptest::prelude::*;
use std::collections::HashMap;

use super::{BuildEdge, CycleDetector, canonicalize_cycle};

fn path(name: &str) -> camino::Utf8PathBuf {
    camino::Utf8PathBuf::from(name)
}

fn build_edge(inputs: &[&str], implicit_deps: &[&str], output: &str) -> BuildEdge {
    BuildEdge {
        action_id: "id".into(),
        inputs: inputs.iter().map(|name| path(name)).collect(),
        implicit_deps: implicit_deps.iter().map(|name| path(name)).collect(),
        explicit_outputs: vec![path(output)],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    }
}

/// Generate a non-empty list of distinct single-character node names.
fn node_names(min: usize, max: usize) -> impl Strategy<Value = Vec<String>> {
    proptest::collection::vec("[a-z]", min..=max).prop_filter("nodes must be unique", |v| {
        let set: std::collections::HashSet<_> = v.iter().collect();
        set.len() == v.len()
    })
}

/// Build a closed cycle from `nodes`: [...nodes, nodes[0]].
fn make_cycle(nodes: &[String]) -> Vec<camino::Utf8PathBuf> {
    let mut cycle: Vec<_> = nodes.iter().map(|s| path(s)).collect();
    cycle.push(path(
        nodes
            .first()
            .expect("node_names generates at least two nodes"),
    ));
    cycle
}

fn check_canonicalize_cycle(input: &[&str], expected: &[&str]) {
    let cycle: Vec<camino::Utf8PathBuf> = input.iter().map(|&s| path(s)).collect();
    let canonical = canonicalize_cycle(cycle);
    let want: Vec<camino::Utf8PathBuf> = expected.iter().map(|&s| path(s)).collect();
    assert_eq!(canonical, want);
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
        let interior = canonical
            .get(..canonical.len().saturating_sub(1))
            .expect("canonicalize_cycle produces at least two nodes");
        let first = canonical
            .first()
            .expect("canonicalize_cycle produces at least one node");
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
fn find_cycle_is_deterministic() {
    let mut targets = HashMap::new();
    targets.insert(path("p"), build_edge(&["q"], &[], "p"));
    targets.insert(path("q"), build_edge(&["p"], &[], "q"));
    targets.insert(path("x"), build_edge(&["y"], &[], "x"));
    targets.insert(path("y"), build_edge(&["x"], &[], "y"));

    let first = CycleDetector::find_cycle(&targets).expect("cycle");
    for _ in 1..100 {
        let cycle = CycleDetector::find_cycle(&targets).expect("cycle");
        assert!(
            cycle == first,
            "find_cycle returned inconsistent results across runs: \
             first={first:?}, got={cycle:?}",
        );
    }
    // Probabilistic guard: 100 runs; `detect` sorts keys for stable traversal.
    tracing::info!("find_cycle returned the same cycle across 100 runs");
}

#[test]
fn canonicalize_cycle_rotates_smallest_node() {
    check_canonicalize_cycle(&["c", "a", "b", "c"], &["a", "b", "c", "a"]);
}

#[test]
fn canonicalize_cycle_handles_reverse_direction() {
    check_canonicalize_cycle(&["c", "b", "a", "c"], &["a", "c", "b", "a"]);
}

#[test]
fn find_cycle_detects_one_of_multiple_disjoint_cycles() {
    let mut targets = HashMap::new();
    targets.insert(path("p"), build_edge(&["q"], &[], "p"));
    targets.insert(path("q"), build_edge(&["p"], &[], "q"));
    targets.insert(path("x"), build_edge(&["y"], &[], "x"));
    targets.insert(path("y"), build_edge(&["x"], &[], "y"));

    assert!(CycleDetector::find_cycle(&targets).is_some());
}

/// Generate a list of `count` distinct node names "n0", "n1", …
fn sequential_nodes(count: usize) -> Vec<String> {
    (0..count).map(|i| format!("n{i}")).collect()
}

/// Build an acyclic chain: n0 → n1 → … → n(count-1) (no back-edges).
fn make_acyclic_chain(nodes: &[String]) -> HashMap<camino::Utf8PathBuf, super::super::BuildEdge> {
    let mut targets = HashMap::new();
    let mut iter = nodes.iter().peekable();
    while let Some(name) = iter.next() {
        let inputs: Vec<&str> = iter.peek().map(|n| n.as_str()).into_iter().collect();
        targets.insert(path(name), build_edge(&inputs, &[], name));
    }
    targets
}

/// Build a cycle graph from the provided node names, starting at node index 0.
///
/// Each node depends on the next name in the slice, and the last node depends
/// on the first, preserving the input order as the cycle order.
fn make_cycle_graph(nodes: &[String]) -> HashMap<camino::Utf8PathBuf, super::super::BuildEdge> {
    let mut targets = HashMap::new();
    let deps = nodes.iter().cycle().skip(1).take(nodes.len());
    for (name, dep) in nodes.iter().zip(deps) {
        targets.insert(path(name), build_edge(&[dep.as_str()], &[], name));
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
}
