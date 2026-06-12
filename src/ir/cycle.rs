//! Cycle detection utilities for the IR target graph.
//!
//! The public entry point is [`analyse`], which accepts the target map
//! (`IrHashMap<Utf8PathBuf, BuildEdge>`) produced by IR lowering and
//! returns a [`CycleDetectionReport`].  The report carries an optional
//! detected cycle — an ordered, canonicalized list of paths — together
//! with any dependencies referenced by a target but absent from the map.
//! `order_only_deps` are intentionally excluded from traversal.
//!
//! Traversal state is managed by the private [`CycleDetector`] struct,
//! which owns the DFS recursion stack and per-node visitation map.
//! Callers drive detection through [`CycleDetector::detect`], which
//! iterates over every node in the target map and delegates depth-first
//! visiting to `visit` and `visit_dependency`.  Detected cycles are
//! normalized by [`canonicalize_cycle`] to produce deterministic error
//! messages regardless of traversal order.  Consumed by
//! [`super::from_manifest`] after the full target map is constructed.

use std::cmp::Ordering;

use camino::{Utf8Path, Utf8PathBuf};

use super::graph::{BuildEdge, IrHashMap};

#[cfg(test)]
#[path = "cycle_property_tests.rs"]
mod cycle_property_tests;

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
/// last element.  `missing_dependencies` lists unresolved dependencies
/// encountered before the first detected cycle.
pub(crate) struct CycleDetectionReport {
    pub(crate) cycle: Option<Vec<Utf8PathBuf>>,
    pub(crate) missing_dependencies: Vec<(Utf8PathBuf, Utf8PathBuf)>,
}

/// Detect cycles and collect missing dependencies in `targets`.
///
/// Performs a depth-first traversal of each [`BuildEdge`]'s `inputs` and
/// `implicit_deps`.  `order_only_deps` are intentionally excluded.
///
/// Returns any detected cycle path and missing dependencies encountered
/// before that cycle.  Missing dependencies emit debug-level tracing events.
pub(crate) fn analyse(targets: &IrHashMap<Utf8PathBuf, BuildEdge>) -> CycleDetectionReport {
    let mut detector = CycleDetector::new(targets);
    let cycle = detector.detect();
    CycleDetectionReport {
        cycle,
        missing_dependencies: detector.missing_dependencies,
    }
}

/// Depth-first cycle detector that owns its traversal state.
///
/// This is a deliberate struct rather than a closure or set of free
/// functions for three reasons:
///
/// 1. **Reset semantics.** [`CycleDetector::detect`] clears `stack`,
///    `states`, and `missing_dependencies` before each run, making
///    repeated calls on the same detector safe and predictable.  A
///    free-function design would require threading that reset contract
///    through every call site.
///
/// 2. **State isolation.** The recursion stack and visitation map are
///    owned entirely by the detector, keeping `visit` and
///    `visit_dependency` focused on traversal logic without lengthening
///    every parameter list.
///
/// 3. **Testability.** Detector property tests call `detect()` directly and
///    inspect `stack` to verify clean unwinding; exposing that verification
///    through `analyse`'s return type alone would widen the public API
///    unnecessarily.
///
/// Create with [`CycleDetector::new`] and drive detection with
/// [`CycleDetector::detect`].
struct CycleDetector<'targets> {
    targets: &'targets IrHashMap<Utf8PathBuf, BuildEdge>,
    stack: Vec<Utf8PathBuf>,
    states: IrHashMap<Utf8PathBuf, VisitState>,
    missing_dependencies: Vec<(Utf8PathBuf, Utf8PathBuf)>,
}

impl CycleDetector<'_> {
    /// Create a new detector borrowing `targets` for the duration of the
    /// traversal.
    fn new(targets: &IrHashMap<Utf8PathBuf, BuildEdge>) -> CycleDetector<'_> {
        CycleDetector {
            targets,
            stack: Vec::new(),
            states: IrHashMap::default(),
            missing_dependencies: Vec::new(),
        }
    }

    /// Walk every node in the target map and return the first cycle found.
    fn detect(&mut self) -> Option<Vec<Utf8PathBuf>> {
        self.states.clear();
        self.stack.clear();
        self.missing_dependencies.clear();

        let mut nodes: Vec<Utf8PathBuf> = self.targets.keys().cloned().collect();
        // Sort keys for deterministic traversal order.  The O(n log n) cost is
        // negligible for typical build graphs (100–10 000 targets) and is
        // outweighed by the benefit of stable, reproducible error messages.
        nodes.sort_by(|left, right| path_cmp(left.as_path(), right.as_path()));
        for node in nodes {
            if self.is_visited(node.as_path()) {
                continue;
            }
            if let Some(cycle) = self.visit(node.as_path()) {
                return Some(cycle);
            }
        }
        None
    }

    /// Return `true` if `node` has been fully visited.
    fn is_visited(&self, node: &Utf8Path) -> bool {
        matches!(
            state_for_path(&self.states, node),
            Some(VisitState::Visited)
        )
    }

    /// Visit `node` depth-first.
    ///
    /// Returns `Some(cycle)` if a back-edge to an in-progress node is
    /// discovered, `None` otherwise.
    fn visit(&mut self, node: &Utf8Path) -> Option<Vec<Utf8PathBuf>> {
        match state_for_path(&self.states, node) {
            Some(VisitState::Visited) => return None,
            Some(VisitState::Visiting) => {
                let idx = self
                    .stack
                    .iter()
                    .position(|n| path_eq(n.as_path(), node))
                    .unwrap_or_else(|| {
                        debug_assert!(false, "visiting node must be on the stack");
                        0
                    });
                let mut cycle: Vec<Utf8PathBuf> = self.stack.iter().skip(idx).cloned().collect();
                cycle.push(node.to_path_buf());
                return Some(canonicalize_cycle(cycle));
            }
            None => {
                self.states.insert(node.to_path_buf(), VisitState::Visiting);
            }
        }

        self.stack.push(node.to_path_buf());

        let cycle = edge_for_path(self.targets, node)
            .into_iter()
            .flat_map(|edge| edge.inputs.iter().chain(&edge.implicit_deps))
            .find_map(|dep| self.visit_dependency(node, dep));

        self.stack.pop();

        if cycle.is_none() {
            self.states.insert(node.to_path_buf(), VisitState::Visited);
        }

        cycle
    }

    #[cfg(test)]
    fn find_cycle(targets: &IrHashMap<Utf8PathBuf, BuildEdge>) -> Option<Vec<Utf8PathBuf>> {
        analyse(targets).cycle
    }

    /// Record `dep` as missing and return `true` if `dep` is absent from the
    /// target map; return `false` if it is present.
    ///
    /// Missing dependencies are also emitted as debug-level tracing events.
    fn record_missing_dependency(&mut self, node: &Utf8Path, dep: &Utf8Path) -> bool {
        if self.target_edge(dep).is_some() {
            return false;
        }

        tracing::debug!(
            missing = %dep,
            dependent = %node,
            "skipping dependency missing from targets during cycle detection",
        );
        self.missing_dependencies
            .push((node.to_path_buf(), dep.to_path_buf()));
        true
    }

    /// Optionally record `dep` as missing, then visit it.
    ///
    /// Returns early with `None` when the dependency is absent from the target
    /// map.
    fn visit_dependency(&mut self, node: &Utf8Path, dep: &Utf8Path) -> Option<Vec<Utf8PathBuf>> {
        if self.record_missing_dependency(node, dep) {
            return None;
        }

        self.visit(dep)
    }

    fn target_edge(&self, node: &Utf8Path) -> Option<&BuildEdge> {
        edge_for_path(self.targets, node)
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
        .min_by(|(_, a), (_, b)| path_cmp(a.as_path(), b.as_path()))
        .map_or(0, |(idx, _)| idx);
    cycle.pop();
    cycle.rotate_left(start);
    if let Some(first) = cycle.first().cloned() {
        cycle.push(first);
    }
    cycle
}

#[cfg(not(kani))]
fn edge_for_path<'targets>(
    targets: &'targets IrHashMap<Utf8PathBuf, BuildEdge>,
    path: &Utf8Path,
) -> Option<&'targets BuildEdge> {
    targets.get(path)
}

#[cfg(kani)]
fn edge_for_path<'targets>(
    targets: &'targets IrHashMap<Utf8PathBuf, BuildEdge>,
    path: &Utf8Path,
) -> Option<&'targets BuildEdge> {
    targets
        .iter()
        .find(|(candidate, _)| path_eq(candidate.as_path(), path))
        .map(|(_, edge)| edge)
}

#[cfg(not(kani))]
fn state_for_path(
    states: &IrHashMap<Utf8PathBuf, VisitState>,
    path: &Utf8Path,
) -> Option<VisitState> {
    states.get(path).copied()
}

#[cfg(kani)]
fn state_for_path(
    states: &IrHashMap<Utf8PathBuf, VisitState>,
    path: &Utf8Path,
) -> Option<VisitState> {
    states
        .iter()
        .find(|(candidate, _)| path_eq(candidate.as_path(), path))
        .map(|(_, state)| *state)
}

#[cfg(not(kani))]
fn path_eq(left: &Utf8Path, right: &Utf8Path) -> bool {
    left == right
}

#[cfg(kani)]
fn path_eq(left: &Utf8Path, right: &Utf8Path) -> bool {
    left.as_str() == right.as_str()
}

#[cfg(not(kani))]
fn path_cmp(left: &Utf8Path, right: &Utf8Path) -> Ordering {
    left.cmp(right)
}

#[cfg(kani)]
fn path_cmp(left: &Utf8Path, right: &Utf8Path) -> Ordering {
    left.as_str().cmp(right.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::collections::HashMap;

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
        assert!(detector.visit(path("a").as_path()).is_none());
        assert_eq!(
            detector.missing_dependencies.as_slice(),
            expected.as_slice()
        );
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
        assert!(detector.is_visited(a.as_path()));
        assert!(detector.is_visited(b.as_path()));
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
        targets.insert(path("a"), build_edge(&["b"], &[], "a"));
        targets.insert(path("b"), build_edge(&["a"], &[], "b"));
        let mut detector = CycleDetector::new(&targets);
        assert!(detector.detect().is_some(), "expected a cycle");
        assert!(
            detector.stack.is_empty(),
            "stack must be empty after cycle detection",
        );
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
}
#[cfg(kani)]
#[path = "cycle_verification.rs"]
mod verification;
