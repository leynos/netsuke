//! Cycle detection utilities for the IR target graph.
//!
//! The public entry point is [`analyse`], which accepts the canonical
//! [`BuildGraph`] produced by IR lowering and
//! returns a [`CycleDetectionReport`].  The report carries an optional
//! detected cycle — an ordered, canonicalized list of paths — together
//! with any dependencies referenced by a target but absent from the map.
//! `order_only_deps` are intentionally excluded from traversal.
//!
//! Traversal state is owned by the private [`CycleDetector`] struct in the
//! sibling `cycle_detector` module; the iteration walks every output alias in
//! the index and resolves its canonical edge through the arena. Detected
//! cycles are normalized by [`support::canonicalize_cycle`] to produce deterministic
//! error messages regardless of traversal order.  Consumed by
//! [`super::from_manifest`] after the full target map is constructed.

use camino::Utf8PathBuf;

use super::graph::BuildGraph;

#[cfg(test)]
#[path = "cycle_property_tests.rs"]
mod cycle_property_tests;

#[path = "cycle_support.rs"]
pub(super) mod support;

#[path = "cycle_detector.rs"]
mod detector;
use self::detector::CycleDetector;
use self::detector::VisitState;

#[cfg(test)]
#[path = "cycle_tests.rs"]
mod tests;

/// The result of a cycle-detection pass over the target graph.
///
/// `cycle` is `Some` when a dependency cycle was found; the vec holds the
/// cycle's nodes in canonical order, with the first node repeated as the
/// last element.  `missing_dependencies` lists unresolved dependencies
/// encountered before the first detected cycle.
pub(crate) struct CycleDetectionReport {
    /// A detected cycle path, when one was found.
    pub(crate) cycle: Option<Vec<Utf8PathBuf>>,
    /// Dependencies referenced by a target but absent from the graph.
    pub(crate) missing_dependencies: Vec<(Utf8PathBuf, Utf8PathBuf)>,
}

/// Detect cycles and collect missing dependencies in `graph`.
///
/// Performs a depth-first traversal of each [`BuildEdge`](super::BuildEdge)'s `inputs` and
/// `implicit_deps`.  `order_only_deps` are intentionally excluded.
///
/// Returns any detected cycle path and missing dependencies encountered
/// before that cycle.  Missing dependencies emit debug-level tracing events.
pub(crate) fn analyse(graph: &BuildGraph) -> CycleDetectionReport {
    let mut detector = CycleDetector::new(graph);
    let cycle = detector.detect();
    CycleDetectionReport {
        cycle,
        missing_dependencies: detector.missing_dependencies,
    }
}

/// Return whether `targets` contains any dependency cycle.
///
/// This drives [`CycleDetector`]'s production traversal in boolean mode.
#[cfg(kani)]
pub(crate) fn contains_cycle(graph: &BuildGraph) -> bool {
    CycleDetector::new(graph).detect_presence()
}

#[cfg(kani)]
#[path = "cycle_verification.rs"]
mod verification;
