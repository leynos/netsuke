//! Depth-first cycle detection over the IR target graph.
//!
//! The [`CycleDetector`] owns the recursion stack and visitation map while it
//! walks the target graph, and [`super`]'s `analyse` entry point drives it.
//! Traversal state is keyed by canonical [`EdgeId`], so an edge with several
//! output aliases is expanded once; stack frames retain the alias that entered
//! each edge so witnesses still name real output paths. Keeping the traversal
//! in its own file bounds the cycle module within the repository's 400-line
//! cap while leaving the canonicalization helpers in [`super::support`].

use camino::{Utf8Path, Utf8PathBuf};

use super::super::graph::{BuildEdge, BuildGraph, EdgeId, IrHashMap};
#[cfg(test)]
use super::analyse;
#[cfg(not(kani))]
use super::support::path_cmp;
use super::support::{canonicalize_cycle, state_for_edge, target_entry_for_path};

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
    /// Lightweight presence probing that avoids allocating cycle paths.
    #[cfg(kani)]
    Presence,
    /// Full enumeration that materializes a cycle path on a back-edge.
    Path,
}

/// The outcome of visiting one node during cycle detection.
#[derive(Debug, Eq, PartialEq)]
pub(super) enum CycleVisitResult {
    /// No cycle was found below this node.
    None,
    /// A cycle exists, without its path.
    #[cfg(kani)]
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

/// One in-progress edge visit, carrying the alias used to enter it.
#[derive(Clone, Copy, Debug)]
pub(super) struct StackFrame<'targets> {
    /// Canonical edge currently being expanded.
    pub(super) edge_id: EdgeId,
    /// Output alias by which the edge was entered.
    pub(super) entry_alias: &'targets Utf8Path,
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
    /// Canonical graph being traversed, borrowed for the traversal.
    graph: &'targets BuildGraph,
    /// DFS stack of in-progress edges, for back-edge extraction. Read by the
    /// property tests to verify clean state between runs.
    pub(super) stack: Vec<StackFrame<'targets>>,
    /// Visitation state per canonical edge.
    states: IrHashMap<EdgeId, VisitState>,
    /// Dependencies referenced but absent from the target map, recorded once
    /// per examined entry under the alias that entered its edge. Read by
    /// [`super::analyse`] to build the detection report.
    pub(super) missing_dependencies: Vec<(Utf8PathBuf, Utf8PathBuf)>,
}

impl<'targets> CycleDetector<'targets> {
    /// Create a new detector borrowing `graph` for the duration of the
    /// traversal.
    pub(super) fn new(graph: &BuildGraph) -> CycleDetector<'_> {
        CycleDetector {
            graph,
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
        #[cfg(test)]
        super::scan_counter::reset_dependency_scans();

        self.detect_targets(search)
    }

    /// Visit every root target, returning on the first detected cycle.
    #[cfg(not(kani))]
    fn detect_targets(&mut self, search: CycleSearch) -> CycleVisitResult {
        // Snapshot borrowed keys (not clones): the `'targets` map outlives
        // the traversal, so the collected references stay valid while `self`
        // is mutated. The collection exists for sorting, not to appease the
        // borrow checker.
        let mut nodes: Vec<&'targets Utf8Path> = self
            .graph
            .output_paths()
            .map(Utf8PathBuf::as_path)
            .collect();
        // Sort keys for deterministic traversal order.  The O(n log n) cost is
        // negligible for typical build graphs (100–10 000 targets) and is
        // outweighed by the benefit of stable, reproducible error messages.
        nodes.sort_by(|left, right| path_cmp(left, right));
        for node in nodes {
            let Some((target, edge_id, _)) = target_entry_for_path(self.graph, node) else {
                continue;
            };
            if matches!(
                state_for_edge(&self.states, edge_id),
                Some(VisitState::Visited)
            ) {
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
        for node in self.graph.output_paths() {
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

    /// Return `true` if the edge producing `node` has been fully visited.
    ///
    /// Production root enumeration checks the state through the entry probe it
    /// already holds; tests and Kani's insertion-order walk resolve a path.
    #[cfg(any(test, kani))]
    pub(super) fn is_visited(&self, node: &Utf8Path) -> bool {
        let Some(edge_id) = self.graph.edge_id_for_output(node) else {
            return false;
        };
        matches!(
            state_for_edge(&self.states, edge_id),
            Some(VisitState::Visited)
        )
    }

    /// Build the [`CycleVisitResult`] for an edge that is currently being
    /// visited — i.e. a back-edge has been discovered.
    ///
    /// In `Path` mode the witness starts with the alias the back edge
    /// requests, then names every alias entered below the target edge, and
    /// closes with the requested alias again. Starting with the target edge's
    /// own entry alias would report an alias the back edge never named.
    ///
    /// In `Presence` mode (Kani only) a lightweight sentinel is returned
    /// without building a path vector.
    fn back_edge_result(
        &self,
        edge_id: EdgeId,
        requested_alias: &'targets Utf8Path,
        search: CycleSearch,
    ) -> CycleVisitResult {
        match search {
            #[cfg(kani)]
            CycleSearch::Presence => CycleVisitResult::Present,
            CycleSearch::Path => CycleVisitResult::Path(canonicalize_cycle(
                self.cycle_from_stack(self.frame_index(edge_id), requested_alias),
            )),
        }
    }

    /// Visit every dependency of a known edge in declaration order,
    /// returning early on the first detected cycle.
    ///
    /// `entry_alias` is the alias that entered the edge; a missing dependency
    /// is recorded under it. `edge` must be borrowed from the `'targets`
    /// lifetime target map so that subsequent mutable borrows of `self`
    /// inside `visit_dependencies` are permitted by the borrow checker.
    fn visit_known_edge(
        &mut self,
        entry_alias: &'targets Utf8Path,
        edge: &'targets BuildEdge,
        search: CycleSearch,
    ) -> CycleVisitResult {
        let cycle = self.visit_dependencies(entry_alias, &edge.inputs, search);
        if cycle.is_cycle() {
            return cycle;
        }
        let implicit_cycle = self.visit_dependencies(entry_alias, &edge.implicit_deps, search);
        if implicit_cycle.is_cycle() {
            return implicit_cycle;
        }
        self.visit_dependencies(entry_alias, &edge.order_only_deps, search)
    }

    /// Visit `node` depth-first.
    ///
    /// Returns a cycle result if a back-edge to an in-progress edge is
    /// discovered. The node is resolved to its canonical edge before the
    /// visitation check, so every alias of a completed edge returns
    /// immediately without rescanning its dependencies.
    pub(super) fn visit(
        &mut self,
        node: &'targets Utf8Path,
        search: CycleSearch,
    ) -> CycleVisitResult {
        let Some((stored_alias, edge_id, edge)) = target_entry_for_path(self.graph, node) else {
            return CycleVisitResult::None;
        };

        match state_for_edge(&self.states, edge_id) {
            Some(VisitState::Visited) => return CycleVisitResult::None,
            Some(VisitState::Visiting) => {
                return self.back_edge_result(edge_id, stored_alias, search);
            }
            None => {
                self.states.insert(edge_id, VisitState::Visiting);
            }
        }

        if matches!(search, CycleSearch::Path) {
            self.stack.push(StackFrame {
                edge_id,
                entry_alias: stored_alias,
            });
        }

        let cycle = self.visit_known_edge(stored_alias, edge, search);

        if matches!(search, CycleSearch::Path) {
            self.stack.pop();
        }

        if !cycle.is_cycle() {
            self.states.insert(edge_id, VisitState::Visited);
        }

        cycle
    }

    /// Return the stack index of `edge_id`, which must be currently visiting.
    fn frame_index(&self, edge_id: EdgeId) -> usize {
        let mut index = 0;
        while index < self.stack.len() {
            if let Some(frame) = self.stack.get(index)
                && frame.edge_id == edge_id
            {
                return index;
            }
            index += 1;
        }
        debug_assert!(false, "visiting edge must be on the stack");
        0
    }

    /// Assemble a cycle witness from the requested alias, every alias entered
    /// below the target edge, and the requested alias again.
    fn cycle_from_stack(&self, start: usize, closing_alias: &Utf8Path) -> Vec<Utf8PathBuf> {
        let mut cycle = Vec::new();
        cycle.push(closing_alias.to_path_buf());
        let mut index = start + 1;
        while index < self.stack.len() {
            if let Some(frame) = self.stack.get(index) {
                cycle.push(frame.entry_alias.to_path_buf());
            }
            index += 1;
        }
        cycle.push(closing_alias.to_path_buf());
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
            super::scan_counter::record_dependency_scan();
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
    pub(super) fn find_cycle(graph: &BuildGraph) -> Option<Vec<Utf8PathBuf>> {
        analyse(graph).cycle
    }

    /// Record `dep` as a missing dependency of the edge entered by `alias`.
    ///
    /// Callers establish absence from the target map before recording.
    fn record_missing_dependency(&mut self, alias: &Utf8Path, dep: &Utf8Path) {
        self.missing_dependencies
            .push((alias.to_path_buf(), dep.to_path_buf()));
    }

    /// Optionally record `dep` as missing, then visit it.
    ///
    /// Returns early with `None` when the dependency is absent from the target
    /// map.
    fn visit_dependency(
        &mut self,
        entry_alias: &'targets Utf8Path,
        dep: &Utf8Path,
        search: CycleSearch,
    ) -> CycleVisitResult {
        let Some((target, _, _)) = target_entry_for_path(self.graph, dep) else {
            if matches!(search, CycleSearch::Path) {
                self.record_missing_dependency(entry_alias, dep);
            }
            return CycleVisitResult::None;
        };

        self.visit(target, search)
    }
}
