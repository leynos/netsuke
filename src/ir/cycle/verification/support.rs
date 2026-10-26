//! Kani-only helpers for verifying cycle canonicalization.

use camino::{Utf8Path, Utf8PathBuf};

use super::super::support::{canonicalize_cycle, canonicalize_cycle_by, path_eq, rotate_index};

/// Hold a generated cycle and the node alphabet used to check it.
pub(super) struct CycleInput {
    /// The unique node identifiers before the cycle is closed.
    interior_ids: Vec<u8>,
    /// The cycle nodes with its starting identifier repeated at the end.
    closed_ids: Vec<u8>,
    /// The exclusive upper bound for symbolic node identifiers.
    alphabet_len: u8,
}

/// Check that the canonicalizer preserves a bounded cycle's structural properties.
pub(super) fn assert_kernel_canonical_properties(input: CycleInput) {
    let input_len = input.closed_ids.len();
    let output = canonicalize_cycle_by(input.closed_ids, compare_u8);

    kani::assert(
        output.len() == input_len,
        "canonical cycle preserves length",
    );
    kani::assert(is_closed_id_cycle(&output), "canonical output is closed");
    assert_multiset_preserved(&input.interior_ids, &output, input.alphabet_len);
    assert_smallest_start(&input.interior_ids, &output);
    kani::assert(
        output_is_rotation(&input.interior_ids, &output),
        "canonical output is a rotation of the input interior",
    );
}

/// Build a symbolic closed cycle with two distinct node identifiers.
pub(super) fn closed_two_node_id_cycle() -> CycleInput {
    let first = symbolic_id(2);
    let second = symbolic_id(2);
    kani::assume(first != second);

    closed_id_cycle(vec![first, second], 2)
}

/// Build a symbolic closed cycle with three distinct node identifiers.
pub(super) fn closed_three_node_id_cycle() -> CycleInput {
    let first = symbolic_id(3);
    let second = symbolic_id(3);
    let third = symbolic_id(3);
    kani::assume(first != second && first != third && second != third);

    closed_id_cycle(vec![first, second, third], 3)
}

/// Build a symbolic closed cycle with four distinct node identifiers.
pub(super) fn closed_four_node_id_cycle() -> CycleInput {
    let first = symbolic_id(4);
    let second = symbolic_id(4);
    let third = symbolic_id(4);
    let fourth = symbolic_id(4);
    kani::assume(
        first != second
            && first != third
            && first != fourth
            && second != third
            && second != fourth
            && third != fourth,
    );

    closed_id_cycle(vec![first, second, third, fourth], 4)
}

/// Close the supplied interior identifiers and retain their alphabet length.
fn closed_id_cycle(interior_ids: Vec<u8>, alphabet_len: u8) -> CycleInput {
    let mut closed_ids = interior_ids.clone();
    if let Some(first) = interior_ids.first().copied() {
        closed_ids.push(first);
    }
    CycleInput {
        interior_ids,
        closed_ids,
        alphabet_len,
    }
}

/// Choose a symbolic identifier within the requested finite alphabet.
fn symbolic_id(alphabet_len: u8) -> u8 {
    let id = kani::any::<u8>();
    kani::assume(id < alphabet_len);
    id
}

/// Check that canonicalization preserves the interior identifier multiset.
fn assert_multiset_preserved(input_ids: &[u8], output: &[u8], alphabet_len: u8) {
    let mut id = 0;
    while id < alphabet_len {
        kani::assert(
            count_id(input_ids, id) == count_output_id(output, id),
            "canonical cycle preserves the interior multiset",
        );
        id += 1;
    }
}

/// Count an identifier in a slice.
fn count_id(ids: &[u8], expected: u8) -> usize {
    let mut count = 0;
    let mut index = 0;
    while index < ids.len() {
        if ids.get(index) == Some(&expected) {
            count += 1;
        }
        index += 1;
    }
    count
}

/// Count an identifier before the closing node in a cycle.
fn count_output_id(ids: &[u8], expected: u8) -> usize {
    let mut count = 0;
    let mut index = 0;
    while index < interior_len(ids) {
        if ids.get(index) == Some(&expected) {
            count += 1;
        }
        index += 1;
    }
    count
}

/// Check that the canonical cycle starts at its smallest interior identifier.
fn assert_smallest_start(input_ids: &[u8], output: &[u8]) {
    if let Some(min_id) = min_id(input_ids) {
        kani::assert(
            output.first() == Some(&min_id),
            "canonical first node is smallest",
        );
    }
}

/// Return the smallest identifier in a slice.
fn min_id(ids: &[u8]) -> Option<u8> {
    let mut index = 0;
    let mut min = None;
    while index < ids.len() {
        if let Some(id) = ids.get(index).copied() {
            min = Some(match min {
                Some(current) if current < id => current,
                _ => id,
            });
        }
        index += 1;
    }
    min
}

/// Return whether the output is a rotation of the input cycle interior.
fn output_is_rotation(input_ids: &[u8], output: &[u8]) -> bool {
    let mut start = 0;
    while start < input_ids.len() {
        if output_matches_rotation(input_ids, output, start) {
            return true;
        }
        start += 1;
    }
    false
}

/// Compare the output against one rotation of the input identifiers.
fn output_matches_rotation(input_ids: &[u8], output: &[u8], start: usize) -> bool {
    let mut offset = 0;
    while offset < input_ids.len() {
        let Some(expected_id) = input_ids
            .get(rotate_index(start, offset, input_ids.len()))
            .copied()
        else {
            return false;
        };
        if output.get(offset) != Some(&expected_id) {
            return false;
        }
        offset += 1;
    }
    true
}

/// Check that path canonicalization agrees with the generic identifier kernel.
pub(super) fn assert_path_wrapper_matches_kernel(first_id: u8, second_id: u8) {
    let expected = canonicalize_cycle_by(vec![first_id, second_id, first_id], compare_u8);
    let output = canonicalize_cycle(vec![
        path_for_id(first_id),
        path_for_id(second_id),
        path_for_id(first_id),
    ]);

    kani::assert(
        output.len() == expected.len(),
        "path wrapper preserves length",
    );
    kani::assert(
        paths_match_ids(&output, &expected),
        "path wrapper matches the generic kernel",
    );
}

/// Return whether each path represents the identifier at the same index.
fn paths_match_ids(paths: &[Utf8PathBuf], ids: &[u8]) -> bool {
    let mut index = 0;
    while index < ids.len() {
        let Some(path) = paths.get(index) else {
            return false;
        };
        let Some(id) = ids.get(index).copied() else {
            return false;
        };
        if !path_matches_id(path, id) {
            return false;
        }
        index += 1;
    }
    paths.len() == ids.len()
}

/// Return whether a path matches the name assigned to an identifier.
fn path_matches_id(path: &Utf8PathBuf, id: u8) -> bool {
    path_eq(path.as_path(), Utf8Path::new(name_for_id(id)))
}

/// Convert an identifier to its representative UTF-8 path.
fn path_for_id(id: u8) -> Utf8PathBuf {
    Utf8PathBuf::from(name_for_id(id))
}

/// Return the canonical short name assigned to an identifier.
fn name_for_id(id: u8) -> &'static str {
    match id {
        0 => "a",
        1 => "b",
        2 => "c",
        _ => "d",
    }
}

/// Compare two identifier values in natural numeric order.
fn compare_u8(left: &u8, right: &u8) -> std::cmp::Ordering {
    left.cmp(right)
}

/// Return the number of non-terminal nodes in a closed cycle.
fn interior_len<T>(cycle: &[T]) -> usize {
    cycle.len().saturating_sub(1)
}

/// Return whether the first and last cycle identifiers are equal.
fn is_closed_id_cycle(cycle: &[u8]) -> bool {
    match (cycle.first(), cycle.last()) {
        (Some(first), Some(last)) => first == last,
        _ => false,
    }
}
