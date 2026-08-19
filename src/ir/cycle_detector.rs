//! Depth-first cycle detection over the IR target graph.
//!
//! The [`CycleDetector`] owns the recursion stack and visitation map while it
//! walks the target graph, and [`super`]'s `analyse` entry point drives it.
//! Keeping the traversal in its own file bounds the cycle module within the
//! repository's 400-line cap while leaving the canonicalization helpers in
//! [`super::support`].

use camino::{Utf8Path, Utf8PathBuf};

use super::super::graph::{BuildEdge, IrHashMap};
#[cfg(test)]
use super::analyse;
#[cfg(not(kani))]
use super::support::path_cmp;
use super::support::{canonicalize_cycle, path_eq, state_for_path, target_entry_for_path};

/// Tracks the visitation state of a node during cycle detection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum VisitState {
    /// The node is on the current DFS stack.
    Visiting,
    /// The node and its dependencies were fully walked.
    Visited,
}

/// The mode a detection pass runs in.
#[derive(Clone, Copy, Debug)]
pub(super) enum CycleSearch {
    #[cfg(kani)]
    /// Lightweight presence probing that avoids allocating cycle paths.
    Presence,
    /// Full enumeration that materializes a cycle path on a back-edge.
    Path,
}

/// The outcome of visiting one node during cycle detection.
#[derive(Debug, Eq, PartialEq)]
pub(super) enum CycleVisitResult {
    /// No cycle was found below this node.
    None,
    #[cfg(kani)]
    /// A cycle exists, without its path.
    Present,
    /// A cycle path, in canonical order with the start repeated last.
    Path(Vec<Utf8PathBuf>),
}

impl CycleVisitResult {
    /// Return whether the result carries a detected cycle.
    const fn is_cycle(&self) -> bool {
        !matches!(self, Self::None)
    }

    /// Convert a path-carrying result into its cycle, if any.
    fn into_path(self) -> Option<Vec<Utf8PathBuf>> {
        match self {
            Self::Path(cycle) => Some(cycle),
            #[cfg(kani)]
            Self::Present => None,
            Self::None => None,
        }
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
pub(super) struct CycleDetector<'targets> {
    /// The target map being traversed, borrowed for the traversal.
    targets: &'targets IrHashMap<Utf8PathBuf, BuildEdge>,
    /// DFS recursion stack of in-progress nodes, for back-edge extraction.
    /// Read by the property tests to verify clean state between runs.
    pub(super) stack: Vec<&'targets Utf8Path>,
    /// Visitation state per node.
    states: IrHashMap<&'targets Utf8Path, VisitState>,
    /// Dependencies referenced but absent from the target map. Read by
    /// [`super::analyse`] to build the detection report.
    pub(super) missing_dependencies: Vec<(Utf8PathBuf, Utf8PathBuf)>,
}

impl<'targets> CycleDetector<'targets> {
    /// Create a new detector borrowing `targets` for the duration of the
    /// traversal.
    pub(super) fn new(targets: &IrHashMap<Utf8PathBuf, BuildEdge>) -> CycleDetector<'_> {
        CycleDetector {
            targets,
            stack: Vec::new(),
            states: IrHashMap::default(),
            missing_dependencies: Vec::new(),
        }
    }

    /// Walk every node in the target map and return the first cycle found.
    pub(super) fn detect(&mut self) -> Option<Vec<Utf8PathBuf>> {
        self.detect_with(CycleSearch::Path).into_path()
    }

    /// Walk every node in the target map and return whether a cycle exists.
    #[cfg(kani)]
    pub(super) fn detect_presence(&mut self) -> bool {
        self.detect_with(CycleSearch::Presence).is_cycle()
    }

    /// Reset the traversal state and walk every target.
    fn detect_with(&mut self, search: CycleSearch) -> CycleVisitResult {
        self.states.clear();
        self.stack.clear();
        self.missing_dependencies.clear();

        self.detect_targets(search)
    }

    /// Visit every root target, returning on the first detected cycle.
    #[cfg(not(kani))]
    fn detect_targets(&mut self, search: CycleSearch) -> CycleVisitResult {
        // Snapshot borrowed keys (not clones): the `'targets` map outlives
        // the traversal, so the collected references stay valid while `self`
        // is mutated. The collection exists for sorting, not to appease the
        // borrow checker.
        let mut nodes: Vec<&'targets Utf8Path> =
            self.targets.keys().map(Utf8PathBuf::as_path).collect();
        // Sort keys for deterministic traversal order.  The O(n log n) cost is
        // negligible for typical build graphs (100–10 000 targets) and is
        // outweighed by the benefit of stable, reproducible error messages.
        nodes.sort_by(|left, right| path_cmp(left, right));
        for node in nodes {
            let Some((target, _)) = target_entry_for_path(self.targets, node) else {
                continue;
            };
            if self.is_visited(target) {
                continue;
            }
            let result = self.visit(target, search);
            if result.is_cycle() {
                return result;
            }
        }
        CycleVisitResult::None
    }

    /// Visit the Kani-flavoured target map, probing for any cycle.
    #[cfg(kani)]
    fn detect_targets(&mut self, search: CycleSearch) -> CycleVisitResult {
        for index in 0..self.targets.len() {
            let Some((node, _)) = self.targets.entry_at(index) else {
                continue;
            };
            if self.is_visited(node.as_path()) {
                continue;
            }
            let result = self.visit(node.as_path(), search);
            if result.is_cycle() {
                return result;
            }
        }
        CycleVisitResult::None
    }

    /// Return `true` if `node` has been fully visited.
    pub(super) fn is_visited(&self, node: &Utf8Path) -> bool {
        matches!(
            state_for_path(&self.states, node),
            Some(VisitState::Visited)
        )
    }

    /// Build the [`CycleVisitResult`] for a node that is currently being
    /// visited — i.e. a back-edge has been discovered.
    ///
    /// In `Path` mode the cycle is extracted from the DFS stack and
    /// canonicalized.  In `Presence` mode (Kani only) a lightweight sentinel
    /// is returned without allocating a path vector.
    fn back_edge_result(&self, node: &'targets Utf8Path, search: CycleSearch) -> CycleVisitResult {
        match search {
            #[cfg(kani)]
            CycleSearch::Presence => CycleVisitResult::Present,
            CycleSearch::Path => CycleVisitResult::Path(canonicalize_cycle(
                self.cycle_from_stack(self.stack_index(node), node),
            )),
        }
    }

    /// Visit the `inputs` and `implicit_deps` of a known edge in order,
    /// returning early on the first detected cycle.
    ///
    /// `edge` must be borrowed from the `'targets`-lifetime target map so
    /// that subsequent mutable borrows of `self` inside `visit_dependencies`
    /// are permitted by the borrow checker.
    fn visit_known_edge(
        &mut self,
        node: &'targets Utf8Path,
        edge: &'targets BuildEdge,
        search: CycleSearch,
    ) -> CycleVisitResult {
        let cycle = self.visit_dependencies(node, &edge.inputs, search);
        if cycle.is_cycle() {
            return cycle;
        }
        self.visit_dependencies(node, &edge.implicit_deps, search)
    }

    /// Visit `node` depth-first.
    ///
    /// Returns a cycle result if a back-edge to an in-progress node is
    /// discovered.
    pub(super) fn visit(
        &mut self,
        node: &'targets Utf8Path,
        search: CycleSearch,
    ) -> CycleVisitResult {
        match state_for_path(&self.states, node) {
            Some(VisitState::Visited) => return CycleVisitResult::None,
            Some(VisitState::Visiting) => return self.back_edge_result(node, search),
            None => {
                self.states.insert(node, VisitState::Visiting);
            }
        }

        if matches!(search, CycleSearch::Path) {
            self.stack.push(node);
        }

        let cycle = match target_entry_for_path(self.targets, node) {
            Some((_, edge)) => self.visit_known_edge(node, edge, search),
            None => CycleVisitResult::None,
        };

        if matches!(search, CycleSearch::Path) {
            self.stack.pop();
        }

        if !cycle.is_cycle() {
            self.states.insert(node, VisitState::Visited);
        }

        cycle
    }

    /// Return the stack index of `node`, which must be currently visiting.
    fn stack_index(&self, node: &Utf8Path) -> usize {
        let mut index = 0;
        while index < self.stack.len() {
            if let Some(candidate) = self.stack.get(index)
                && path_eq(candidate, node)
            {
                return index;
            }
            index += 1;
        }
        debug_assert!(false, "visiting node must be on the stack");
        0
    }

    /// Assemble the cycle path from a stack start to `node`, end-inclusive.
    fn cycle_from_stack(&self, start: usize, node: &Utf8Path) -> Vec<Utf8PathBuf> {
        let mut cycle = Vec::new();
        let mut index = start;
        while index < self.stack.len() {
            if let Some(path) = self.stack.get(index) {
                cycle.push(path.to_path_buf());
            }
            index += 1;
        }
        cycle.push(node.to_path_buf());
        cycle
    }

    /// Visit every dependency in `dependencies`, returning on a cycle.
    fn visit_dependencies(
        &mut self,
        node: &'targets Utf8Path,
        dependencies: &[Utf8PathBuf],
        search: CycleSearch,
    ) -> CycleVisitResult {
        let mut index = 0;
        while index < dependencies.len() {
            let Some(dependency) = dependencies.get(index) else {
                index += 1;
                continue;
            };
            let result = self.visit_dependency(node, dependency.as_path(), search);
            if result.is_cycle() {
                return result;
            }
            index += 1;
        }
        CycleVisitResult::None
    }

    /// Run a full detection pass and return the cycle, for tests.
    #[cfg(test)]
    pub(super) fn find_cycle(
        targets: &IrHashMap<Utf8PathBuf, BuildEdge>,
    ) -> Option<Vec<Utf8PathBuf>> {
        analyse(targets).cycle
    }

    /// Record `dep` as missing and return `true` if `dep` is absent from the
    /// target map; return `false` if it is present.
    ///
    /// Missing dependencies are also emitted as debug-level tracing events.
    fn record_missing_dependency(&mut self, node: &Utf8Path, dep: &Utf8Path) {
        tracing::debug!(
            missing = %dep,
            dependent = %node,
            "skipping dependency missing from targets during cycle detection",
        );
        self.missing_dependencies
            .push((node.to_path_buf(), dep.to_path_buf()));
    }

    /// Optionally record `dep` as missing, then visit it.
    ///
    /// Returns early with `None` when the dependency is absent from the target
    /// map.
    fn visit_dependency(
        &mut self,
        node: &'targets Utf8Path,
        dep: &Utf8Path,
        search: CycleSearch,
    ) -> CycleVisitResult {
        let Some((target, _)) = target_entry_for_path(self.targets, dep) else {
            if matches!(search, CycleSearch::Path) {
                self.record_missing_dependency(node, dep);
            }
            return CycleVisitResult::None;
        };

        self.visit(target, search)
    }
}
