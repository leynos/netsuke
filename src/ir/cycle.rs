//! Cycle detection utilities for the IR target graph.

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

pub(crate) fn analyse(targets: &HashMap<Utf8PathBuf, BuildEdge>) -> CycleDetectionReport {
    let mut detector = CycleDetector::new(targets);
    let mut cycle = None;
    for node in targets.keys() {
        if detector.is_visited(node) {
            continue;
        }
        if let Some(found) = detector.visit(node.clone()) {
            cycle = Some(found);
            break;
        }
    }
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
    fn new(targets: &HashMap<Utf8PathBuf, BuildEdge>) -> CycleDetector<'_> {
        CycleDetector {
            targets,
            stack: Vec::new(),
            states: HashMap::new(),
            missing_dependencies: Vec::new(),
        }
    }

    fn is_visited(&self, node: &Utf8PathBuf) -> bool {
        matches!(self.states.get(node), Some(VisitState::Visited))
    }

    fn visit(&mut self, node: Utf8PathBuf) -> Option<Vec<Utf8PathBuf>> {
        match self.states.get(&node) {
            Some(VisitState::Visited) => return None,
            Some(VisitState::Visiting) => {
                let idx = self
                    .stack
                    .iter()
                    .position(|n| n == &node)
                    .unwrap_or_else(|| {
                        debug_assert!(false, "visiting node must be on the stack");
                        0
                    });
                let mut cycle: Vec<Utf8PathBuf> = self.stack.iter().skip(idx).cloned().collect();
                cycle.push(node);
                return Some(canonicalize_cycle(cycle));
            }
            None => {
                self.states.insert(node.clone(), VisitState::Visiting);
            }
        }

        self.stack.push(node.clone());

        if let Some(cycle) = self
            .targets
            .get(&node)
            .into_iter()
            .flat_map(|edge| edge.inputs.iter().chain(&edge.implicit_deps))
            .find_map(|dep| self.visit_dependency(&node, dep))
        {
            return Some(cycle);
        }

        self.stack.pop();
        self.states.insert(node, VisitState::Visited);
        None
    }

    #[cfg(test)]
    fn missing_dependencies(&self) -> &[(Utf8PathBuf, Utf8PathBuf)] {
        &self.missing_dependencies
    }

    #[cfg(test)]
    fn find_cycle(targets: &HashMap<Utf8PathBuf, BuildEdge>) -> Option<Vec<Utf8PathBuf>> {
        analyse(targets).cycle
    }
}

impl CycleDetector<'_> {
    fn record_missing_dependency(&mut self, node: &Utf8PathBuf, dep: &Utf8PathBuf) -> bool {
        if self.targets.contains_key(dep) {
            return false;
        }

        tracing::debug!(
            missing = %dep,
            dependent = %node,
            "skipping dependency missing from targets during cycle detection",
        );
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
    let (prefix, suffix) = cycle.split_at_mut(len);
    prefix.rotate_left(start);
    if let (Some(first), Some(last)) = (prefix.first().cloned(), suffix.first_mut()) {
        *last = first;
    }
    cycle
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn assert_missing_deps(
        targets: &HashMap<Utf8PathBuf, BuildEdge>,
        expected: &[(Utf8PathBuf, Utf8PathBuf)],
    ) {
        let mut detector = CycleDetector::new(targets);
        assert!(detector.visit(path("a")).is_none());
        assert_eq!(detector.missing_dependencies(), expected);
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
        assert!(detector.visit(a.clone()).is_none());
        assert!(detector.is_visited(&a));
        assert!(detector.is_visited(&b));
        assert!(
            detector.stack.is_empty(),
            "stack should be empty after complete traversal",
        );
    }

    #[test]
    fn cycle_detector_records_missing_dependencies() {
        let mut targets = HashMap::new();
        targets.insert(path("a"), build_edge(&["b"], &[], "a"));
        assert_missing_deps(&targets, &[(path("a"), path("b"))]);
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
    fn find_cycle_identifies_mixed_input_and_implicit_dependency_cycle() {
        let mut targets = HashMap::new();
        targets.insert(path("a"), build_edge(&["b"], &[], "a"));
        targets.insert(path("b"), build_edge(&[], &["c"], "b"));
        targets.insert(path("c"), build_edge(&["a"], &[], "c"));

        let cycle = CycleDetector::find_cycle(&targets).expect("cycle");
        assert_eq!(cycle, vec![path("a"), path("b"), path("c"), path("a")]);
    }

    #[test]
    fn cycle_detector_records_missing_implicit_dependencies() {
        let mut targets = HashMap::new();
        targets.insert(path("a"), build_edge(&["b"], &["missing"], "a"));
        targets.insert(path("b"), build_edge(&[], &[], "b"));
        assert_missing_deps(&targets, &[(path("a"), path("missing"))]);
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
        let cycle = vec![path("c"), path("a"), path("b"), path("c")];
        let canonical = canonicalize_cycle(cycle);
        let expected = vec![path("a"), path("b"), path("c"), path("a")];
        assert_eq!(canonical, expected);
    }

    #[test]
    fn canonicalize_cycle_handles_reverse_direction() {
        let cycle = vec![path("c"), path("b"), path("a"), path("c")];
        let canonical = canonicalize_cycle(cycle);
        let expected = vec![path("a"), path("c"), path("b"), path("a")];
        assert_eq!(canonical, expected);
    }
}
