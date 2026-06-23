//! Kani harnesses for bounded IR cycle-handling properties.

use super::*;

/// Prove a self-dependency reports a cycle and no missing dependency.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(5)]
fn self_dependency_reports_cycle() {
    let mut targets = IrHashMap::default();
    targets.insert(path("a"), edge("a", deps("a"), Vec::new()));
    kani::assume(targets.len() == 1);

    kani::assert(contains_cycle(&targets), "self-dependency reports a cycle");
}

/// Prove a two-node cycle is detected when `a` is inserted first.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(5)]
fn two_node_cycle_reports_cycle_a_first() {
    let mut targets = IrHashMap::default();
    targets.insert(path("a"), edge("a", deps("b"), Vec::new()));
    targets.insert(path("b"), edge("b", deps("a"), Vec::new()));
    kani::assume(targets.len() == 2);

    kani::assert(contains_cycle(&targets), "two-node cycle is rejected");
}

/// Prove a two-node cycle is detected when `b` is inserted first.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(5)]
fn two_node_cycle_reports_cycle_b_first() {
    let mut targets = IrHashMap::default();
    targets.insert(path("b"), edge("b", deps("a"), Vec::new()));
    targets.insert(path("a"), edge("a", deps("b"), Vec::new()));
    kani::assume(targets.len() == 2);

    kani::assert(contains_cycle(&targets), "two-node cycle is rejected");
}

/// Assert that the given target graph contains no cycle.
fn assert_no_cycle(targets: &IrHashMap<Utf8PathBuf, BuildEdge>, _msg: &'static str) {
    kani::assert(
        !contains_cycle(targets),
        "missing dependency is not a cycle",
    );
}

/// Prove an absent direct dependency is not cyclic.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(6)]
fn direct_missing_dependency_does_not_report_cycle() {
    let mut targets = IrHashMap::default();
    targets.insert(path("a"), edge("a", deps("c"), Vec::new()));
    kani::assume(targets.len() == 1);

    assert_no_cycle(&targets, "direct missing dependency is not a cycle");
}

/// Prove an absent dependency beyond a present target is not cyclic.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(6)]
fn transitive_missing_dependency_does_not_report_cycle() {
    let mut targets = IrHashMap::default();
    targets.insert(path("a"), edge("a", deps("b"), Vec::new()));
    targets.insert(path("b"), edge("b", deps("c"), Vec::new()));
    kani::assume(targets.len() == 2);

    assert_no_cycle(&targets, "transitive missing dependency is not a cycle");
}

/// Prove two-node canonicalization preserves the canonical cycle contract.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(6)]
fn canonicalize_two_node_cycle_is_canonical() {
    assert_kernel_canonical_properties(closed_two_node_id_cycle());
}

/// Prove three-node canonicalization preserves the canonical cycle contract.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(6)]
fn canonicalize_three_node_cycle_is_canonical() {
    assert_kernel_canonical_properties(closed_three_node_id_cycle());
}

/// Prove four-node canonicalization preserves the canonical cycle contract.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(6)]
fn canonicalize_four_node_cycle_is_canonical() {
    assert_kernel_canonical_properties(closed_four_node_id_cycle());
}

/// Prove the path wrapper agrees with the generic kernel for two-node cycles.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(6)]
fn canonicalize_path_wrapper_matches_u8_kernel_for_two_nodes() {
    assert_path_wrapper_matches_kernel(0, 1);
    assert_path_wrapper_matches_kernel(1, 0);
}

struct CycleInput {
    interior_ids: Vec<u8>,
    closed_ids: Vec<u8>,
    alphabet_len: u8,
}

fn assert_kernel_canonical_properties(input: CycleInput) {
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

fn closed_two_node_id_cycle() -> CycleInput {
    let first = symbolic_id(2);
    let second = symbolic_id(2);
    kani::assume(first != second);

    closed_id_cycle(vec![first, second], 2)
}

fn closed_three_node_id_cycle() -> CycleInput {
    let first = symbolic_id(3);
    let second = symbolic_id(3);
    let third = symbolic_id(3);
    kani::assume(first != second && first != third && second != third);

    closed_id_cycle(vec![first, second, third], 3)
}

fn closed_four_node_id_cycle() -> CycleInput {
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

fn symbolic_id(alphabet_len: u8) -> u8 {
    let id = kani::any::<u8>();
    kani::assume(id < alphabet_len);
    id
}

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

fn assert_smallest_start(input_ids: &[u8], output: &[u8]) {
    if let Some(min_id) = min_id(input_ids) {
        kani::assert(
            output.first() == Some(&min_id),
            "canonical first node is smallest",
        );
    }
}

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

fn assert_path_wrapper_matches_kernel(first_id: u8, second_id: u8) {
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

fn path_matches_id(path: &Utf8PathBuf, id: u8) -> bool {
    path_eq(path.as_path(), Utf8Path::new(name_for_id(id)))
}

fn path_for_id(id: u8) -> Utf8PathBuf {
    Utf8PathBuf::from(name_for_id(id))
}

fn name_for_id(id: u8) -> &'static str {
    match id {
        0 => "a",
        1 => "b",
        2 => "c",
        _ => "d",
    }
}

fn compare_u8(left: &u8, right: &u8) -> std::cmp::Ordering {
    left.cmp(right)
}

fn interior_len<T>(cycle: &[T]) -> usize {
    cycle.len().saturating_sub(1)
}

fn is_closed_id_cycle(cycle: &[u8]) -> bool {
    cycle.first() == cycle.last()
}

fn edge(output: &str, inputs: Vec<Utf8PathBuf>, implicit_deps: Vec<Utf8PathBuf>) -> BuildEdge {
    BuildEdge {
        action_id: "id".to_owned(),
        inputs,
        implicit_deps,
        explicit_outputs: vec![path(output)],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    }
}

fn deps(dependency: &str) -> Vec<Utf8PathBuf> {
    vec![path(dependency)]
}

fn path(name: &str) -> Utf8PathBuf {
    Utf8PathBuf::from(name)
}
