//! Match the application recorder's bounded metric label vocabularies.
//!
//! Keep these queries private to the recorder; callers supply its reviewed
//! label shapes, and matching never expands the accepted names or values.

use metrics::Key;

/// Match `key` against any of the `expected` label shapes exactly.
///
/// One counter may be recorded under more than one bounded label shape: the
/// `which` resolution counter carries a `category` label only when it fails,
/// so its success series have two labels and its failure series three. Each
/// shape is admitted independently and a series matching none is still
/// rejected, so the alternation widens the vocabulary without letting an
/// unreviewed label set through.
pub(super) fn any_exact_labels(key: &Key, expected: &[&[(&str, &[&str])]]) -> bool {
    expected.iter().any(|shape| exact_labels(key, shape))
}

/// Match `key` against the `expected` labels exactly.
///
/// Mirrors the exact-match assertions in [`super::tests`] so production and
/// tests share one label vocabulary.
pub(super) fn exact_labels(key: &Key, expected: &[(&str, &[&str])]) -> bool {
    let labels: Vec<_> = key.labels().collect();
    labels.len() == expected.len()
        && labels
            .iter()
            .zip(expected)
            .all(|(label, &(name, values))| label.key() == name && values.contains(&label.value()))
}
