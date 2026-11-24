//! Deterministic, Kani-friendly sorting and comparison helpers.
//!
//! Kept separate from [`super::super`]'s manifest-lowering modules so the
//! duplicate-output and rule-resolution paths stay within the repository's
//! 400-line cap. The implementations avoid `std::slice::sort` and full-width
//! comparisons so the Kani harnesses can verify the orderings with bounded
//! symbolic input.

#[cfg(all(test, not(kani)))]
use std::cell::Cell;

use camino::Utf8PathBuf;

use super::super::super::cycle::support::path_eq;

#[cfg(kani)]
use super::super::super::cycle::support::first_byte_cmp;
#[cfg(not(kani))]
use super::super::super::cycle::support::path_cmp;

/// Sort `values` in place with a stable insertion sort driven by `cmp`.
///
/// Deliberately dependency-free and deterministic for the Kani harnesses.
#[cfg(not(kani))]
pub(super) fn insertion_sort_by<T, F>(values: &mut [T], cmp: F)
where
    F: Fn(&T, &T) -> std::cmp::Ordering,
{
    let mut index = 1;
    while index < values.len() {
        let mut sorted_index = index;
        while sorted_index > 0 {
            let swap = values
                .get(sorted_index)
                .zip(values.get(sorted_index - 1))
                .is_some_and(|(cur, prev)| cmp(cur, prev) == std::cmp::Ordering::Less);
            if !swap {
                break;
            }
            values.swap(sorted_index, sorted_index - 1);
            sorted_index -= 1;
        }
        index += 1;
    }
}

/// How many rule names a multi-name rejection reports before summarising.
///
/// The diagnostic exists to tell a manifest author which selector went wrong,
/// not to echo a selector back. A cap keeps the message useful and the
/// rejection's cost linear in the selector's length, however large the list.
pub(super) const MAX_REPORTED_RULE_SELECTORS: usize = 8;

/// Select the `limit` lexically smallest names, in ascending order.
///
/// The result is the stable ascending order of the retained names, so a
/// selector small enough to report in full renders exactly as a full sort
/// would, duplicates and their relative order included. Only retained names
/// are cloned: at most `limit` of them, after one pass that makes at most
/// `names.len() * limit` comparisons.
pub(super) fn smallest_names(names: &[String], limit: usize) -> Vec<String> {
    let capacity = if names.len() < limit {
        names.len()
    } else {
        limit
    };
    let mut kept: Vec<&str> = Vec::with_capacity(capacity);
    let mut index = 0;
    // Indexed iteration keeps the Kani harnesses in
    // `crate::ir::from_manifest::verification` tractable; an iterator chain
    // here defeats their loop unwinding bounds.
    while let Some(name) = names.get(index) {
        insert_bounded(&mut kept, name.as_str(), limit);
        index += 1;
    }
    let mut selected = Vec::with_capacity(kept.len());
    let mut position = 0;
    while let Some(name) = kept.get(position) {
        selected.push((*name).to_owned());
        position += 1;
    }
    selected
}

/// Insert `name` into the bounded ascending prefix `kept`.
///
/// The scan stops at the first entry `name` sorts before, so equal names keep
/// their input order. A full prefix drops its largest entry only when `name`
/// sorts before it; otherwise `name` is discarded without being cloned.
fn insert_bounded<'a>(kept: &mut Vec<&'a str>, name: &'a str, limit: usize) {
    let mut position = 0;
    while let Some(candidate) = kept.get(position) {
        if string_cmp(name, candidate) == std::cmp::Ordering::Less {
            break;
        }
        position += 1;
    }
    if kept.len() < limit {
        kept.insert(position, name);
    } else if position < limit {
        kept.pop();
        kept.insert(position, name);
    }
}

/// Order two strings for deterministic rule-name sorting.
#[cfg(not(kani))]
fn string_cmp(left: &str, right: &str) -> std::cmp::Ordering {
    record_string_comparison();
    left.cmp(right)
}

/// Order two strings for deterministic rule-name sorting.
///
/// The Kani build compares only first bytes, matching the harnesses'
/// single-byte symbolic names.
#[cfg(kani)]
fn string_cmp(left: &str, right: &str) -> std::cmp::Ordering {
    first_byte_cmp(left, right)
}

#[cfg(all(test, not(kani)))]
thread_local! {
    /// Rule-name comparisons made by the current thread.
    static STRING_COMPARISONS: Cell<usize> = const { Cell::new(0) };
}

/// Count one rule-name comparison in test builds.
#[cfg(all(test, not(kani)))]
fn record_string_comparison() {
    STRING_COMPARISONS.with(|count| count.set(count.get() + 1));
}

/// Skip rule-name comparison counting outside test builds.
///
/// No Kani twin: the Kani `string_cmp` compares only first bytes and never
/// records, so nothing in that build would call this.
#[cfg(all(not(test), not(kani)))]
const fn record_string_comparison() {}

/// Reset the test-only rule-name comparison count on this thread.
///
/// Sorting tests read a difference, so a count left over from an earlier test
/// on the same thread would make the measurement meaningless.
#[cfg(all(test, not(kani)))]
pub(super) fn reset_string_comparisons() {
    STRING_COMPARISONS.with(|count| count.set(0));
}

/// Report how many rule-name comparisons this thread has made.
#[cfg(all(test, not(kani)))]
pub(super) fn string_comparisons() -> usize {
    STRING_COMPARISONS.with(Cell::get)
}

/// Sort duplicate output paths in place.
#[cfg(not(kani))]
pub(super) fn sort_paths(paths: &mut [Utf8PathBuf]) {
    insertion_sort_by(paths, |left, right| {
        path_cmp(left.as_path(), right.as_path())
    });
}

/// Sort duplicate output paths in place.
///
/// A no-op in the Kani build.
#[cfg(kani)]
pub(super) fn sort_paths(_paths: &mut [Utf8PathBuf]) {}

/// Report whether `output` already appears in the seen-output slice.
pub(super) fn has_seen_output(seen: &[&Utf8PathBuf], output: &Utf8PathBuf) -> bool {
    let mut index = 0;
    while index < seen.len() {
        if let Some(candidate) = seen.get(index)
            && path_eq(candidate.as_path(), output.as_path())
        {
            return true;
        }
        index += 1;
    }
    false
}
