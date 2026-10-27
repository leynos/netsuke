//! Kani-only helpers for verifying cycle canonicalization.

use camino::{Utf8Path, Utf8PathBuf};

use super::super::support::{canonicalize_cycle, canonicalize_cycle_by, path_eq, rotate_index};

/// Distinguish finite proof-node identifiers from bounds and slice indices.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CycleNodeId(u8);

/// Bound the finite alphabet from which symbolic proof nodes are selected.
#[derive(Clone, Copy, Debug)]
struct CycleAlphabetLen(u8);

/// Identify one candidate rotation of a cycle's interior nodes.
#[derive(Clone, Copy, Debug)]
struct RotationStart(usize);

/// Hold a generated cycle and the node alphabet used to check it.
pub(super) struct CycleInput {
    /// The unique node identifiers before the cycle is closed.
    interior_ids: Vec<CycleNodeId>,
    /// The cycle nodes with its starting identifier repeated at the end.
    closed_ids: Vec<CycleNodeId>,
    /// The exclusive upper bound for symbolic node identifiers.
    alphabet_len: CycleAlphabetLen,
}

/// Check that the canonicalizer preserves a bounded cycle's structural properties.
pub(super) fn assert_kernel_canonical_properties(input: CycleInput) {
    let input_len = input.closed_ids.len();
    let output = canonicalize_cycle_by(input.closed_ids, compare_cycle_node_id);

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
    let alphabet_len = CycleAlphabetLen(2);
    let first = symbolic_id(alphabet_len);
    let second = symbolic_id(alphabet_len);
    kani::assume(first != second);

    closed_id_cycle(vec![first, second], alphabet_len)
}

/// Build a symbolic closed cycle with three distinct node identifiers.
pub(super) fn closed_three_node_id_cycle() -> CycleInput {
    let alphabet_len = CycleAlphabetLen(3);
    let first = symbolic_id(alphabet_len);
    let second = symbolic_id(alphabet_len);
    let third = symbolic_id(alphabet_len);
    kani::assume(first != second && first != third && second != third);

    closed_id_cycle(vec![first, second, third], alphabet_len)
}

/// Build a symbolic closed cycle with four distinct node identifiers.
pub(super) fn closed_four_node_id_cycle() -> CycleInput {
    let alphabet_len = CycleAlphabetLen(4);
    let first = symbolic_id(alphabet_len);
    let second = symbolic_id(alphabet_len);
    let third = symbolic_id(alphabet_len);
    let fourth = symbolic_id(alphabet_len);
    kani::assume(
        first != second
            && first != third
            && first != fourth
            && second != third
            && second != fourth
            && third != fourth,
    );

    closed_id_cycle(vec![first, second, third, fourth], alphabet_len)
}

/// Close the supplied interior identifiers and retain their alphabet length.
fn closed_id_cycle(interior_ids: Vec<CycleNodeId>, alphabet_len: CycleAlphabetLen) -> CycleInput {
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
fn symbolic_id(alphabet_len: CycleAlphabetLen) -> CycleNodeId {
    let value = kani::any::<u8>();
    kani::assume(value < alphabet_len.0);
    CycleNodeId(value)
}

/// Check that canonicalization preserves the interior identifier multiset.
fn assert_multiset_preserved(
    input_ids: &[CycleNodeId],
    output: &[CycleNodeId],
    alphabet_len: CycleAlphabetLen,
) {
    let mut value = 0;
    while value < alphabet_len.0 {
        kani::assert(
            count_id(input_ids, CycleNodeId(value)) == count_output_id(output, CycleNodeId(value)),
            "canonical cycle preserves the interior multiset",
        );
        value += 1;
    }
}

/// Count an identifier in a slice.
fn count_id(ids: &[CycleNodeId], expected: CycleNodeId) -> usize {
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
fn count_output_id(ids: &[CycleNodeId], expected: CycleNodeId) -> usize {
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
fn assert_smallest_start(input_ids: &[CycleNodeId], output: &[CycleNodeId]) {
    if let Some(min_id) = min_id(input_ids) {
        kani::assert(
            output.first() == Some(&min_id),
            "canonical first node is smallest",
        );
    }
}

/// Return the smallest identifier in a slice.
fn min_id(ids: &[CycleNodeId]) -> Option<CycleNodeId> {
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
fn output_is_rotation(input_ids: &[CycleNodeId], output: &[CycleNodeId]) -> bool {
    let mut start = 0;
    while start < input_ids.len() {
        if output_matches_rotation(input_ids, output, RotationStart(start)) {
            return true;
        }
        start += 1;
    }
    false
}

/// Compare the output against one rotation of the input identifiers.
fn output_matches_rotation(
    input_ids: &[CycleNodeId],
    output: &[CycleNodeId],
    start: RotationStart,
) -> bool {
    let mut offset = 0;
    while offset < input_ids.len() {
        let Some(expected_id) = input_ids
            .get(rotate_index(start.0, offset, input_ids.len()))
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
pub(super) fn assert_path_wrapper_matches_kernel() {
    assert_path_wrapper_matches_kernel_for(CycleNodeId(0), CycleNodeId(1));
    assert_path_wrapper_matches_kernel_for(CycleNodeId(1), CycleNodeId(0));
}

/// Check one pair of proof-node identifiers against the path wrapper.
fn assert_path_wrapper_matches_kernel_for(first_id: CycleNodeId, second_id: CycleNodeId) {
    let expected =
        canonicalize_cycle_by(vec![first_id, second_id, first_id], compare_cycle_node_id);
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
fn paths_match_ids(paths: &[Utf8PathBuf], ids: &[CycleNodeId]) -> bool {
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
fn path_matches_id(path: &Utf8PathBuf, id: CycleNodeId) -> bool {
    path_eq(path.as_path(), Utf8Path::new(name_for_id(id)))
}

/// Convert an identifier to its representative UTF-8 path.
fn path_for_id(id: CycleNodeId) -> Utf8PathBuf {
    Utf8PathBuf::from(name_for_id(id))
}

/// Return the canonical short name assigned to an identifier.
fn name_for_id(id: CycleNodeId) -> &'static str {
    match id.0 {
        0 => "a",
        1 => "b",
        2 => "c",
        _ => "d",
    }
}

/// Compare two proof-node identifiers in natural numeric order.
fn compare_cycle_node_id(left: &CycleNodeId, right: &CycleNodeId) -> std::cmp::Ordering {
    left.cmp(right)
}

/// Return the number of non-terminal nodes in a closed cycle.
fn interior_len<T>(cycle: &[T]) -> usize {
    cycle.len().saturating_sub(1)
}

/// Return whether the first and last cycle identifiers are equal.
fn is_closed_id_cycle(cycle: &[CycleNodeId]) -> bool {
    match (cycle.first(), cycle.last()) {
        (Some(first), Some(last)) => first == last,
        _ => false,
    }
}
