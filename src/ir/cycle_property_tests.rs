use proptest::prelude::*;

use super::super::canonicalize_cycle;
use super::path;

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
