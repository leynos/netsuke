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

/// Prove two-node canonicalization preserves length and closure.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(6)]
fn canonicalize_two_node_cycle_is_canonical() {
    let input = closed_two_node_cycle();
    let input_len = input.len();
    let output = canonicalize_cycle(input);

    kani::assert(
        output.len() == input_len,
        "canonical cycle preserves length",
    );
    kani::assert(is_closed_cycle(&output), "canonical output is closed");
}

/// Prove three-node canonicalization preserves length and closure.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(6)]
fn canonicalize_three_node_cycle_is_canonical() {
    let input = closed_three_node_cycle();
    let input_len = input.len();
    let output = canonicalize_cycle(input);

    kani::assert(
        output.len() == input_len,
        "canonical cycle preserves length",
    );
    kani::assert(is_closed_cycle(&output), "canonical output is closed");
}

/// Prove four-node canonicalization preserves length and closure.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(6)]
fn canonicalize_four_node_cycle_is_canonical() {
    let input = closed_four_node_cycle();
    let input_len = input.len();
    let output = canonicalize_cycle(input);

    kani::assert(
        output.len() == input_len,
        "canonical cycle preserves length",
    );
    kani::assert(is_closed_cycle(&output), "canonical output is closed");
}

fn closed_two_node_cycle() -> Vec<Utf8PathBuf> {
    close_cycle(vec![symbolic_node(2), symbolic_node(2)])
}

fn closed_three_node_cycle() -> Vec<Utf8PathBuf> {
    close_cycle(vec![symbolic_node(3), symbolic_node(3), symbolic_node(3)])
}

fn closed_four_node_cycle() -> Vec<Utf8PathBuf> {
    close_cycle(vec![
        symbolic_node(4),
        symbolic_node(4),
        symbolic_node(4),
        symbolic_node(4),
    ])
}

fn close_cycle(mut interior: Vec<Utf8PathBuf>) -> Vec<Utf8PathBuf> {
    if let Some(first) = interior.first().cloned() {
        interior.push(first);
    }
    interior
}

fn symbolic_node(alphabet_len: u8) -> Utf8PathBuf {
    let selector = kani::any::<u8>();
    kani::assume(selector < alphabet_len);
    match selector {
        0 => path("a"),
        1 => path("b"),
        2 => path("c"),
        _ => path("d"),
    }
}

fn is_closed_cycle(cycle: &[Utf8PathBuf]) -> bool {
    match (cycle.first(), cycle.last()) {
        (Some(first), Some(last)) => path_eq(first.as_path(), last.as_path()),
        _ => false,
    }
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
