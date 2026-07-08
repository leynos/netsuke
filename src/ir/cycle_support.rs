//! Support helpers for cycle detection: target and state lookups, path
//! comparison, and canonical rotation of detected cycles.
//!
//! Kani-cfg variants provide bounded, index-based implementations that the
//! verification harnesses can reason about symbolically.

use std::cmp::Ordering;

use camino::{Utf8Path, Utf8PathBuf};

use super::super::graph::{BuildEdge, IrHashMap};
use super::VisitState;

#[cfg(not(kani))]
pub(super) fn target_entry_for_path<'targets>(
    targets: &'targets IrHashMap<Utf8PathBuf, BuildEdge>,
    path: &Utf8Path,
) -> Option<(&'targets Utf8Path, &'targets BuildEdge)> {
    targets
        .get_key_value(path)
        .map(|(target, edge)| (target.as_path(), edge))
}

#[cfg(kani)]
pub(super) fn target_entry_for_path<'targets>(
    targets: &'targets IrHashMap<Utf8PathBuf, BuildEdge>,
    path: &Utf8Path,
) -> Option<(&'targets Utf8Path, &'targets BuildEdge)> {
    let mut index = 0;
    while index < targets.len() {
        if let Some((candidate, edge)) = targets.entry_at(index) {
            if path_eq(candidate.as_path(), path) {
                return Some((candidate.as_path(), edge));
            }
        }
        index += 1;
    }
    None
}

/// Return the index of the smallest node in `cycle[0..len]`.
pub(super) fn find_rotation_start_by<T>(
    cycle: &[T],
    len: usize,
    compare: fn(&T, &T) -> Ordering,
) -> usize {
    let mut start = 0;
    let mut index = 1;
    while index < len {
        if let (Some(candidate), Some(current)) = (cycle.get(index), cycle.get(start))
            && compare(candidate, current) == Ordering::Less
        {
            start = index;
        }
        index += 1;
    }
    start
}

/// Build a canonical, closed cycle by rotating `cycle` so that the node at
/// `start` appears first, then appending that node again to close the cycle.
pub(super) fn rotate_cycle_by<T: Clone>(cycle: &[T], start: usize, len: usize) -> Vec<T> {
    let mut canonical = Vec::with_capacity(len + 1);
    let mut offset = 0;
    while offset < len {
        if let Some(node) = cycle.get(rotate_index(start, offset, len)) {
            canonical.push(node.clone());
        }
        offset += 1;
    }
    if let Some(first) = canonical.first().cloned() {
        canonical.push(first);
    }
    canonical
}

/// Rotate `cycle` so that its smallest node appears first, then re-close it by
/// appending the first node.
///
/// The input must contain at least two nodes; the first and last node are
/// expected to be identical (the standard DFS cycle representation).
pub(super) fn canonicalize_cycle_by<T: Clone>(
    mut cycle: Vec<T>,
    compare: fn(&T, &T) -> Ordering,
) -> Vec<T> {
    debug_assert!(
        cycle.len() >= 2,
        "cycle detection should yield at least two nodes",
    );
    // Runtime guard: `debug_assert` does not fire in release builds, so guard
    // against the underflow `cycle.len() - 1` would otherwise cause on an empty
    // or single-element cycle. Such cycles are structurally impossible from the
    // detector, so returning the input unchanged is a pure defensive measure.
    if cycle.len() < 2 {
        return cycle;
    }
    let len = cycle.len() - 1;
    let start = find_rotation_start_by(&cycle, len, compare);
    cycle.pop();
    rotate_cycle_by(&cycle, start, len)
}

/// Rotate `cycle` so that the lexicographically smallest node appears
/// first, then re-close it by appending the first node.
///
/// The input must contain at least two nodes; the first and last node are
/// expected to be identical (the standard DFS cycle representation).
pub(super) fn canonicalize_cycle(cycle: Vec<Utf8PathBuf>) -> Vec<Utf8PathBuf> {
    canonicalize_cycle_by(cycle, compare_cycle_paths)
}

/// Compare two path-backed cycle nodes using the production path ordering.
pub(super) fn compare_cycle_paths(left: &Utf8PathBuf, right: &Utf8PathBuf) -> Ordering {
    path_cmp(left.as_path(), right.as_path())
}

pub(super) const fn rotate_index(start: usize, offset: usize, len: usize) -> usize {
    let index = start + offset;
    if index >= len { index - len } else { index }
}

#[cfg(not(kani))]
pub(super) fn state_for_path(
    states: &IrHashMap<&Utf8Path, VisitState>,
    path: &Utf8Path,
) -> Option<VisitState> {
    states.get(path).copied()
}

#[cfg(kani)]
pub(super) fn state_for_path(
    states: &IrHashMap<&Utf8Path, VisitState>,
    path: &Utf8Path,
) -> Option<VisitState> {
    let mut index = 0;
    while index < states.len() {
        if let Some((candidate, state)) = states.entry_at(index) {
            if path_eq(candidate, path) {
                return Some(*state);
            }
        }
        index += 1;
    }
    None
}

#[cfg(not(kani))]
pub(super) fn path_eq(left: &Utf8Path, right: &Utf8Path) -> bool {
    left == right
}

#[cfg(kani)]
pub(super) fn path_eq(left: &Utf8Path, right: &Utf8Path) -> bool {
    let left = left.as_str().as_bytes();
    let right = right.as_str().as_bytes();
    left.len() == 1 && right.len() == 1 && left[0] == right[0]
}

#[cfg(not(kani))]
pub(super) fn path_cmp(left: &Utf8Path, right: &Utf8Path) -> Ordering {
    left.cmp(right)
}

#[cfg(kani)]
pub(super) fn path_cmp(left: &Utf8Path, right: &Utf8Path) -> Ordering {
    let left = left.as_str().as_bytes();
    let right = right.as_str().as_bytes();
    match (left.first(), right.first()) {
        (Some(left), Some(right)) => left.cmp(right),
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}
