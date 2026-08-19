//! Cycle detection utilities for the IR target graph.
//!
//! The public entry point is [`analyse`], which accepts the target map
//! (`IrHashMap<Utf8PathBuf, BuildEdge>`) produced by IR lowering and
//! returns a [`CycleDetectionReport`].  The report carries an optional
//! detected cycle — an ordered, canonicalized list of paths — together
//! with any dependencies referenced by a target but absent from the map.
//! `order_only_deps` are intentionally excluded from traversal.
//!
//! Traversal state is owned by the private [`CycleDetector`] struct in the
//! sibling `cycle_detector` module; the iteration walks every node in the
//! target map and delegates depth-first visiting to the detector.  Detected
//! cycles are normalized by [`canonicalize_cycle`] to produce deterministic
//! error messages regardless of traversal order.  Consumed by
//! [`super::from_manifest`] after the full target map is constructed.

use camino::Utf8PathBuf;

use super::graph::{BuildEdge, IrHashMap};

#[cfg(test)]
#[path = "cycle_property_tests.rs"]
mod cycle_property_tests;

#[path = "cycle_support.rs"]
mod support;
// The test and Kani harnesses reach these through `super::*`/`super::Name`;
// expose them only in those builds so the production build neither warns nor
// carries their weight.
#[cfg(any(test, kani))]
use support::canonicalize_cycle;
#[cfg(any(test, kani))]
use support::canonicalize_cycle_by;
#[cfg(kani)]
use support::path_eq;
#[cfg(test)]
use support::target_entry_for_path;

#[path = "cycle_detector.rs"]
mod detector;
// Plain re-import: the bindings stay private to `cycle` but remain reachable
// from its `#[cfg(test)]`/`#[cfg(kani)]` children through `super::*`.
use self::detector::CycleDetector;
use self::detector::VisitState;
#[cfg(any(test, kani))]
use self::detector::{CycleSearch, CycleVisitResult};

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

/// Return whether `targets` contains any dependency cycle.
///
/// This drives [`CycleDetector`]'s production traversal in boolean mode.
#[cfg(kani)]
pub(crate) fn contains_cycle(targets: &IrHashMap<Utf8PathBuf, BuildEdge>) -> bool {
    CycleDetector::new(targets).detect_presence()
}

#[cfg(kani)]
#[path = "cycle_verification.rs"]
mod verification;
