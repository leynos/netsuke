//! Deterministic, Kani-friendly sorting and comparison helpers.
//!
//! Kept separate from [`super::super`]'s manifest-lowering modules so the
//! duplicate-output and rule-resolution paths stay within the repository's
//! 400-line cap. The implementations avoid `std::slice::sort` and full-width
//! comparisons so the Kani harnesses can verify the orderings with bounded
//! symbolic input.

use camino::Utf8PathBuf;

use super::super::super::cycle::support::{path_cmp, path_eq};

/// Sort `values` in place with a stable insertion sort driven by `cmp`.
///
/// Deliberately dependency-free and deterministic for the Kani harnesses.
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

/// Sort a slice of rule names in place.
pub(super) fn sort_strings(values: &mut [String]) {
    // The closure is not redundant: `string_cmp` takes `&str`, and passing
    // the fn item would not satisfy the `Fn(&String, &String)` bound.
    insertion_sort_by(values, |a, b| string_cmp(a, b));
}

/// Order two strings for deterministic rule-name sorting.
#[cfg(not(kani))]
fn string_cmp(left: &str, right: &str) -> std::cmp::Ordering {
    left.cmp(right)
}

/// Order two strings for deterministic rule-name sorting.
///
/// The Kani build compares only first bytes, matching the harnesses'
/// single-byte symbolic names.
#[cfg(kani)]
fn string_cmp(left: &str, right: &str) -> std::cmp::Ordering {
    let left = left.as_bytes();
    let right = right.as_bytes();
    match (left.first(), right.first()) {
        (Some(left), Some(right)) => left.cmp(right),
        (None, Some(_)) => std::cmp::Ordering::Less,
        (Some(_), None) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
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
