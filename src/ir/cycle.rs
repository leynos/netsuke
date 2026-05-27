//! Cycle detection utilities for the IR target graph.
//!
//! Implements [`CycleDetector`], which performs a depth-first traversal of
//! [`BuildEdge`] `inputs` and `implicit_deps` to detect circular dependencies
//! and record missing dependency references.  `order_only_deps` are
//! intentionally excluded from traversal.  Consumed by
//! [`super::from_manifest`] after the full target map is constructed.

use std::collections::HashMap;

use camino::Utf8PathBuf;

use super::BuildEdge;

/// Tracks the visitation state of a node during cycle detection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VisitState {
    Visiting,
    Visited,
}

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

struct CycleDetector<'targets> {
    targets: &'targets HashMap<Utf8PathBuf, BuildEdge>,
    stack: Vec<Utf8PathBuf>,
    states: HashMap<Utf8PathBuf, VisitState>,
    missing_dependencies: Vec<(Utf8PathBuf, Utf8PathBuf)>,
}

impl CycleDetector<'_> {
    fn record_missing_dependency(&mut self, node: &Utf8PathBuf, dep: &Utf8PathBuf) -> bool {
        if self.targets.contains_key(dep) {
            return false;
        }

        self.missing_dependencies.push((node.clone(), dep.clone()));
        true
    }

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
    fn record_missing_dependency(&mut self, node: &Utf8PathBuf, dep: &Utf8PathBuf) -> bool {
        if self.targets.contains_key(dep) {
            return false;
        }

        self.missing_dependencies.push((node.clone(), dep.clone()));
        true
    }

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
}
