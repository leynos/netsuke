//! Cycle detection utilities for the IR target graph.
//!
//! The public entry point is [`analyse`], which accepts the target map
//! (`HashMap<Utf8PathBuf, BuildEdge>`) produced by IR lowering and
//! returns a [`CycleDetectionReport`].  The report carries an optional
//! detected cycle — an ordered, canonicalised list of paths — together
//! with any dependencies referenced by a target but absent from the map.
//! `order_only_deps` are intentionally excluded from traversal.
//!
//! Traversal state is managed by the private [`CycleDetector`] struct,
//! which owns the DFS recursion stack and per-node visitation map.
//! Callers drive detection through [`CycleDetector::detect`], which
//! iterates over every node in the target map and delegates depth-first
//! visiting to `visit` and `visit_dependency`.  Detected cycles are
//! normalised by [`canonicalize_cycle`] to produce deterministic error
//! messages regardless of traversal order.
//! Consumed by [`super::from_manifest`] after the full target map is
//! constructed.

use std::collections::HashMap;

use camino::Utf8PathBuf;

use super::BuildEdge;

/// Tracks the visitation state of a node during cycle detection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VisitState {
    Visiting,
    Visited,
}

/// The result of a cycle-detection pass over the target graph.
///
/// `cycle` is `Some` when a dependency cycle was found; the vec holds the
/// cycle's nodes in canonical order, with the first node repeated as the
/// last element.  `missing_dependencies` lists every `(dependent, dep)`
/// pair where `dep` is referenced but absent from the target map.
pub(crate) struct CycleDetectionReport {
    pub(crate) cycle: Option<Vec<Utf8PathBuf>>,
    pub(crate) missing_dependencies: Vec<(Utf8PathBuf, Utf8PathBuf)>,
}

/// Detect cycles and collect missing dependencies in `targets`.
///
/// Performs a depth-first traversal of each [`BuildEdge`]'s `inputs` and
/// `implicit_deps`.  `order_only_deps` are intentionally excluded.
///
/// Returns a [`CycleDetectionReport`] containing any detected cycle path and
/// all dependency references that could not be resolved to a build target.
/// This function does **not** emit any log events; the caller is responsible
/// for logging the reported data.
pub(crate) fn analyse(targets: &HashMap<Utf8PathBuf, BuildEdge>) -> CycleDetectionReport {
    let mut detector = CycleDetector::new(targets);
    let cycle = detector.detect();
    CycleDetectionReport {
        cycle,
        missing_dependencies: detector.missing_dependencies,
    }
}

/// Depth-first cycle detector that owns its traversal state.
///
/// Create with [`CycleDetector::new`] and drive detection with
/// [`CycleDetector::detect`].
struct CycleDetector<'targets> {
    targets: &'targets HashMap<Utf8PathBuf, BuildEdge>,
    stack: Vec<Utf8PathBuf>,
    states: HashMap<Utf8PathBuf, VisitState>,
    missing_dependencies: Vec<(Utf8PathBuf, Utf8PathBuf)>,
}

impl CycleDetector<'_> {
    /// Record `dep` as missing and return `true` if `dep` is absent from the
    /// target map; return `false` if it is present.
    fn record_missing_dependency(&mut self, node: &Utf8PathBuf, dep: &Utf8PathBuf) -> bool {
        if self.targets.contains_key(dep) {
            return false;
        }

        self.missing_dependencies.push((node.clone(), dep.clone()));
        true
    }

    /// Optionally record `dep` as missing, then visit it.
    ///
    /// Returns early with `None` when the dependency is absent from the target
    /// map.
    fn visit_dependency(
        &mut self,
        node: &Utf8PathBuf,
        dep: &Utf8PathBuf,
    ) -> Option<Vec<Utf8PathBuf>> {
        if self.record_missing_dependency(node, dep) {
            return None;
        }

        self.visit(dep.clone())
    }
}

impl CycleDetector<'_> {
    /// Record `dep` as missing and return `true` if `dep` is absent from the
    /// target map; return `false` if it is present.
    fn record_missing_dependency(&mut self, node: &Utf8PathBuf, dep: &Utf8PathBuf) -> bool {
        if self.targets.contains_key(dep) {
            return false;
        }

        self.missing_dependencies.push((node.clone(), dep.clone()));
        true
    }

    /// Optionally record `dep` as missing, then visit it.
    ///
    /// Returns early with `None` when the dependency is absent from the target
    /// map.
    fn visit_dependency(
        &mut self,
        node: &Utf8PathBuf,
        dep: &Utf8PathBuf,
    ) -> Option<Vec<Utf8PathBuf>> {
        if self.record_missing_dependency(node, dep) {
            return None;
        }

        self.visit(dep.clone())
    }
}

/// Rotate `cycle` so that the lexicographically smallest node appears
/// first, then re-close it by appending the first node.
///
/// The input must contain at least two nodes; the first and last node are
/// expected to be identical (the standard DFS cycle representation).
fn canonicalize_cycle(mut cycle: Vec<Utf8PathBuf>) -> Vec<Utf8PathBuf> {
    debug_assert!(
        cycle.len() >= 2,
        "cycle detection should yield at least two nodes",
    );
    let len = cycle.len() - 1;
    let start = cycle
        .iter()
        .take(len)
        .enumerate()
        .min_by(|(_, a), (_, b)| a.cmp(b))
        .map_or(0, |(idx, _)| idx);
    cycle.pop();
    cycle.rotate_left(start);
    if let Some(first) = cycle.first().cloned() {
        cycle.push(first);
    }
    cycle
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn path(name: &str) -> Utf8PathBuf {
        Utf8PathBuf::from(name)
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

    struct MissingDepsCase<'a> {
        primary_inputs: &'a [&'a str],
        primary_implicit_deps: &'a [&'a str],
        extra_targets: &'a [(&'a str, &'a [&'a str], &'a [&'a str])],
        expected: &'a [(&'a str, &'a str)],
    }

    fn assert_missing_deps(case: &MissingDepsCase<'_>) {
        let mut targets = HashMap::new();
        targets.insert(
            path("a"),
            build_edge(case.primary_inputs, case.primary_implicit_deps, "a"),
        );
        for (output, inputs, implicit_deps) in case.extra_targets {
            targets.insert(path(output), build_edge(inputs, implicit_deps, output));
        }
        let expected: Vec<_> = case
            .expected
            .iter()
            .map(|(dependent, missing)| (path(dependent), path(missing)))
            .collect();
        let mut detector = CycleDetector::new(&targets);
        assert!(detector.visit(path("a")).is_none());
        assert_eq!(detector.missing_dependencies(), expected.as_slice());
    }

    fn next_cycle_index(index: usize, cycle_len: usize) -> usize {
        if index + 1 == cycle_len { 0 } else { index + 1 }
    }

    fn insert_cycle_edge(
        targets: &mut HashMap<Utf8PathBuf, BuildEdge>,
        index: usize,
        cycle_len: usize,
        implicit_index: usize,
    ) {
        let output = format!("n{index}");
        let dep = format!("n{}", next_cycle_index(index, cycle_len));
        let edge = if index == implicit_index {
            build_edge(&[], &[&dep], &output)
        } else {
            build_edge(&[&dep], &[], &output)
        };
        targets.insert(output.into(), edge);
    }

    fn assert_bounded_cycle_detected(cycle_len: usize, implicit_index: usize) {
        let mut targets = HashMap::new();
        for index in 0..cycle_len {
            insert_cycle_edge(&mut targets, index, cycle_len, implicit_index);
        }

        assert!(
            CycleDetector::find_cycle(&targets).is_some(),
            "expected cycle with length {cycle_len} and implicit edge at {implicit_index}",
        );
    }

    #[test]
    fn cycle_detector_detects_self_edge_cycle() {
        let mut targets = HashMap::new();
        targets.insert(path("a"), build_edge(&["a"], &[], "a"));

        let cycle = CycleDetector::find_cycle(&targets).expect("cycle");
        assert_eq!(cycle, vec![path("a"), path("a")]);
    }

    #[test]
    fn cycle_detector_marks_nodes_visited_after_traversal() {
        let mut targets = HashMap::new();
        let a = path("a");
        let b = path("b");
        targets.insert(a.clone(), build_edge(&["b"], &[], "a"));
        targets.insert(b.clone(), build_edge(&[], &[], "b"));

        let mut detector = CycleDetector::new(&targets);
        assert!(detector.detect().is_none());
        assert!(detector.is_visited(&a));
        assert!(detector.is_visited(&b));
        assert!(
            detector.stack.is_empty(),
            "stack should be empty after complete traversal",
        );
    }

    #[rstest]
    #[case::explicit_dependency(MissingDepsCase {
        primary_inputs: &["b"],
        primary_implicit_deps: &[],
        extra_targets: &[],
        expected: &[("a", "b")],
    })]
    #[case::implicit_dependency(MissingDepsCase {
        primary_inputs: &["b"],
        primary_implicit_deps: &["missing"],
        extra_targets: &[("b", &[], &[])],
        expected: &[("a", "missing")],
    })]
    fn cycle_detector_records_missing_dependencies(#[case] case: MissingDepsCase<'_>) {
        assert_missing_deps(&case);
    }

    #[test]
    fn find_cycle_identifies_cycle() {
        let mut targets = HashMap::new();
        targets.insert(path("a"), build_edge(&["b"], &[], "a"));
        targets.insert(path("b"), build_edge(&["a"], &[], "b"));

        let cycle = CycleDetector::find_cycle(&targets).expect("cycle");
        assert_eq!(cycle, vec![path("a"), path("b"), path("a")]);
    }

    #[test]
    fn find_cycle_identifies_implicit_dependency_cycle() {
        let mut targets = HashMap::new();
        targets.insert(path("a"), build_edge(&[], &["b"], "a"));
        targets.insert(path("b"), build_edge(&[], &["a"], "b"));

        let cycle = CycleDetector::find_cycle(&targets).expect("cycle");
        assert_eq!(cycle, vec![path("a"), path("b"), path("a")]);
    }

    #[test]
    fn cycle_detector_stack_is_empty_after_cycle_detected() {
        let mut targets = HashMap::new();
        targets.insert(path("a"), build_edge(&["b"], "a"));
        targets.insert(path("b"), build_edge(&["a"], "b"));

        let mut detector = CycleDetector::new(&targets);
        assert!(detector.detect().is_some(), "expected a cycle");
        assert!(
            detector.stack.is_empty(),
            "stack must be empty after cycle detection",
        );
    }

    fn check_canonicalize_cycle(input: &[&str], expected: &[&str]) {
        let cycle: Vec<Utf8PathBuf> = input.iter().map(|&s| path(s)).collect();
        let canonical = canonicalize_cycle(cycle);
        let want: Vec<Utf8PathBuf> = expected.iter().map(|&s| path(s)).collect();
        assert_eq!(canonical, want);
    }

    #[test]
    fn find_cycle_identifies_mixed_input_and_implicit_dependency_cycle() {
        let mut targets = HashMap::new();
        targets.insert(path("a"), build_edge(&["b"], &[], "a"));
        targets.insert(path("b"), build_edge(&[], &["c"], "b"));
        targets.insert(path("c"), build_edge(&["a"], &[], "c"));

        let cycle = CycleDetector::find_cycle(&targets).expect("cycle");
        assert_eq!(cycle, vec![path("a"), path("b"), path("c"), path("a")]);
    }

    #[test]
    fn bounded_cycles_through_inputs_or_implicit_deps_are_detected() {
        let cases = (2..=5).flat_map(|cycle_len| {
            (0..cycle_len).map(move |implicit_index| (cycle_len, implicit_index))
        });
        for (cycle_len, implicit_index) in cases {
            assert_bounded_cycle_detected(cycle_len, implicit_index);
        }
    }

    #[test]
    fn canonicalize_cycle_rotates_smallest_node() {
        check_canonicalize_cycle(&["c", "a", "b", "c"], &["a", "b", "c", "a"]);
    }

    #[test]
    fn canonicalize_cycle_handles_reverse_direction() {
        check_canonicalize_cycle(&["c", "b", "a", "c"], &["a", "c", "b", "a"]);
    }

    mod property_tests {
        use proptest::prelude::*;

        use super::super::canonicalize_cycle;
        use super::path;

        /// Generate a non-empty list of distinct single-character node names.
        fn node_names(min: usize, max: usize) -> impl Strategy<Value = Vec<String>> {
            proptest::collection::vec("[a-z]", min..=max).prop_filter(
                "nodes must be unique",
                |v| {
                    let set: std::collections::HashSet<_> = v.iter().collect();
                    set.len() == v.len()
                },
            )
        }

        /// Build a closed cycle from `nodes`: [...nodes, nodes[0]].
        fn make_cycle(nodes: &[String]) -> Vec<camino::Utf8PathBuf> {
            let mut cycle: Vec<_> = nodes.iter().map(|s| path(s)).collect();
            cycle.push(path(&nodes[0]));
            cycle
        }

        proptest! {
            /// Canonicalisation is idempotent: applying it twice yields the
            /// same result as applying it once.
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

            /// The first node in the canonical form is lexicographically <=
            /// every other non-terminal node.
            #[test]
            fn canonical_first_node_is_smallest(nodes in node_names(2, 10)) {
                let canonical = canonicalize_cycle(make_cycle(&nodes));
                let interior = &canonical[..canonical.len() - 1];
                let first = &canonical[0];
                for node in interior {
                    prop_assert!(first <= node);
                }
            }

            /// The canonical form is closed: first and last elements are
            /// equal.
            #[test]
            fn canonical_cycle_is_closed(nodes in node_names(2, 10)) {
                let canonical = canonicalize_cycle(make_cycle(&nodes));
                prop_assert_eq!(canonical.first(), canonical.last());
            }
        }
    }
}
