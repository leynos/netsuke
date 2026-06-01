use proptest::prelude::*;
use std::collections::HashMap;

use super::super::{canonicalize_cycle, CycleDetector};
use super::{build_edge, path};

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
    /// Canonicalisation is idempotent: applying it twice yields the same
    /// result as applying it once.
    #[test]
    fn canonicalize_is_idempotent(nodes in node_names(2, 10)) {
        let cycle = make_cycle(&nodes);
        let once = canonicalize_cycle(cycle.clone());
        let twice = canonicalize_cycle(once.clone());
        prop_assert_eq!(once, twice);
    }

    /// All rotations of a cycle canonicalise to the same sequence.
    #[test]
    fn all_rotations_canonicalise_identically(nodes in node_names(2, 8)) {
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
