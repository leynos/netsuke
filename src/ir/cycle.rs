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

struct CycleDetector<'a> {
    targets: &'a HashMap<Utf8PathBuf, BuildEdge>,
    stack: Vec<Utf8PathBuf>,
    states: HashMap<Utf8PathBuf, VisitState>,
    missing_dependencies: Vec<(Utf8PathBuf, Utf8PathBuf)>,
}

impl<'a> CycleDetector<'a> {
    fn new(targets: &'a HashMap<Utf8PathBuf, BuildEdge>) -> Self {
        Self {
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

        if let Some(edge) = self.targets.get(&node) {
            for dep in &edge.inputs {
                if !self.targets.contains_key(dep) {
                    tracing::debug!(
                        missing = %dep,
                        dependent = %node,
                        "skipping dependency missing from targets during cycle detection",
                    );
                    self.missing_dependencies.push((node.clone(), dep.clone()));
                    continue;
                }

                if let Some(cycle) = self.visit(dep.clone()) {
                    return Some(cycle);
                }
            }
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
    fn find_cycle(targets: &'a HashMap<Utf8PathBuf, BuildEdge>) -> Option<Vec<Utf8PathBuf>> {
        analyse(targets).cycle
    }
}

fn canonicalize_cycle(mut cycle: Vec<Utf8PathBuf>) -> Vec<Utf8PathBuf> {
    if cycle.len() < 2 {
        return cycle;
    }
    let len = cycle.len() - 1;
    let start = cycle
        .iter()
        .take(len)
        .enumerate()
        .min_by(|(_, a), (_, b)| a.cmp(b))
        .map_or(0, |(idx, _)| idx);
    let (prefix, suffix) = cycle.split_at_mut(len);
    prefix.rotate_left(start);
    if let (Some(first), Some(slot)) = (prefix.first().cloned(), suffix.first_mut()) {
        slot.clone_from(&first);
    }
    cycle
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(name: &str) -> Utf8PathBuf {
        Utf8PathBuf::from(name)
    }

    fn build_edge(inputs: &[&str], output: &str) -> BuildEdge {
        BuildEdge {
            action_id: "id".into(),
            inputs: inputs.iter().map(|name| path(name)).collect(),
            explicit_outputs: vec![path(output)],
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            phony: false,
            always: false,
        }
    }

    #[test]
    fn cycle_detector_detects_self_edge_cycle() {
        let mut targets = HashMap::new();
        targets.insert(path("a"), build_edge(&["a"], "a"));

        let cycle = CycleDetector::find_cycle(&targets).expect("cycle");
        assert_eq!(cycle, vec![path("a"), path("a")]);
    }

    #[test]
    fn cycle_detector_marks_nodes_visited_after_traversal() {
        let mut targets = HashMap::new();
        let a = path("a");
        let b = path("b");
        targets.insert(a.clone(), build_edge(&["b"], "a"));
        targets.insert(b.clone(), build_edge(&[], "b"));

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
        targets.insert(path("a"), build_edge(&["b"], "a"));

        let mut detector = CycleDetector::new(&targets);
        assert!(detector.visit(path("a")).is_none());

        assert_eq!(detector.missing_dependencies(), &[(path("a"), path("b"))],);
    }

    #[test]
    fn find_cycle_identifies_cycle() {
        let mut targets = HashMap::new();
        targets.insert(path("a"), build_edge(&["b"], "a"));
        targets.insert(path("b"), build_edge(&["a"], "b"));

        let cycle = CycleDetector::find_cycle(&targets).expect("cycle");
        assert_eq!(cycle, vec![path("a"), path("b"), path("a")]);
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
