//! Unit tests for cycle detection and canonicalization.
use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;

use super::super::graph::{BuildEdge, BuildGraph};
use super::detector::{CycleDetector, CycleSearch, CycleVisitResult};
use super::support::target_entry_for_path;
use rstest::rstest;

fn path(name: &str) -> Utf8PathBuf {
    Utf8PathBuf::from(name)
}
fn build_edge(inputs: &[&str], implicit_deps: &[&str], output: &str) -> BuildEdge {
    BuildEdge {
        action_id: "id".into(),
        inputs: inputs.iter().map(|name| path(name)).collect(),
        implicit_deps: implicit_deps.iter().map(|name| path(name)).collect(),
        dependency_order: crate::ir::DependencyOrder::Parallel,
        explicit_outputs: vec![path(output)],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    }
}
struct MissingDepsCase<'a> {
    primary_inputs: &'a [&'a str],
    primary_implicit_deps: &'a [&'a str],
    extra_targets: &'a [(&'a str, &'a [&'a str], &'a [&'a str])],
    expected: &'a [(&'a str, &'a str)],
}
fn assert_missing_deps(case: &MissingDepsCase<'_>) -> Result<()> {
    let mut graph = BuildGraph::default();
    graph.insert_edge(build_edge(
        case.primary_inputs,
        case.primary_implicit_deps,
        "a",
    ))?;
    for (output, inputs, implicit_deps) in case.extra_targets {
        graph.insert_edge(build_edge(inputs, implicit_deps, output))?;
    }
    let expected: Vec<_> = case
        .expected
        .iter()
        .map(|(dependent, missing)| (path(dependent), path(missing)))
        .collect();
    let mut detector = CycleDetector::new(&graph);
    let (target, _) = target_entry_for_path(&graph, path("a").as_path())
        .context("primary target should exist")?;
    let visit = detector.visit(target, CycleSearch::Path);
    ensure!(
        visit == CycleVisitResult::None,
        "expected no cycle, got {visit:?}"
    );
    ensure!(
        detector.missing_dependencies.as_slice() == expected.as_slice(),
        "missing dependencies {:?} did not match {expected:?}",
        detector.missing_dependencies
    );
    Ok(())
}
fn next_cycle_index(index: usize, cycle_len: usize) -> usize {
    if index + 1 == cycle_len { 0 } else { index + 1 }
}
fn insert_cycle_edge(
    graph: &mut BuildGraph,
    index: usize,
    cycle_len: usize,
    implicit_index: usize,
) {
    let output = format!("n{index}");
    let dep = format!("n{}", next_cycle_index(index, cycle_len));
    let edge = if index == implicit_index {
        build_edge(&[], &[&dep], &output)
    } else {
        build_edge(&[&dep], &[], &output)
    };
    assert!(
        graph.insert_edge(edge).is_ok(),
        "test graph output aliases must be unique",
    );
}
fn assert_bounded_cycle_detected(cycle_len: usize, implicit_index: usize) {
    let mut graph = BuildGraph::default();
    for index in 0..cycle_len {
        insert_cycle_edge(&mut graph, index, cycle_len, implicit_index);
    }
    assert!(
        CycleDetector::find_cycle(&graph).is_some(),
        "expected cycle with length {cycle_len} and implicit edge at {implicit_index}",
    );
}
#[test]
fn cycle_detector_detects_self_edge_cycle() {
    let mut graph = BuildGraph::default();
    graph
        .insert_edge(build_edge(&["a"], &[], "a"))
        .expect("test graph output aliases must be unique");
    let cycle = CycleDetector::find_cycle(&graph).expect("cycle");
    assert_eq!(cycle, vec![path("a"), path("a")]);
}
#[test]
fn cycle_detector_marks_nodes_visited_after_traversal() {
    let mut graph = BuildGraph::default();
    let a = path("a");
    let b = path("b");
    graph
        .insert_edge(build_edge(&["b"], &[], "a"))
        .expect("test graph output aliases must be unique");
    graph
        .insert_edge(build_edge(&[], &[], "b"))
        .expect("test graph output aliases must be unique");
    let mut detector = CycleDetector::new(&graph);
    assert!(detector.detect().is_none());
    assert!(detector.is_visited(a.as_path()));
    assert!(detector.is_visited(b.as_path()));
    assert!(
        detector.stack.is_empty(),
        "stack should be empty after complete traversal",
    );
}
#[rstest]
#[case::explicit_dependency(MissingDepsCase {
    primary_inputs: &["b"],
    primary_implicit_deps: &[],
    extra_targets: &[],
    expected: &[("a", "b")],
})]
#[case::implicit_dependency(MissingDepsCase {
    primary_inputs: &["b"],
    primary_implicit_deps: &["missing"],
    extra_targets: &[("b", &[], &[])],
    expected: &[("a", "missing")],
})]
fn cycle_detector_records_missing_dependencies(#[case] case: MissingDepsCase<'_>) -> Result<()> {
    assert_missing_deps(&case)
}
/// One dependency-cycle scenario: the edges to insert and the cycle they form.
struct CycleCase<'a> {
    /// Each edge tuple is `(inputs, implicit_deps, output)`.
    edges: &'a [(&'a [&'a str], &'a [&'a str], &'a str)],
    /// The expected canonical cycle, with the first node repeated last.
    expected: &'a [&'a str],
}

/// Insert every edge in `case` and assert the detected cycle matches.
///
/// Reports failures as errors rather than panicking: a free helper is not
/// a `#[test]` body, where the gates deny both `expect` and `assert!` in a
/// `Result`-returning function.
fn assert_cycle(case: &CycleCase<'_>) -> Result<()> {
    let mut graph = BuildGraph::default();
    for (inputs, implicit_deps, output) in case.edges {
        graph
            .insert_edge(build_edge(inputs, implicit_deps, output))
            .context("test graph output aliases must be unique")?;
    }
    let cycle = CycleDetector::find_cycle(&graph).context("cycle")?;
    let expected: Vec<_> = case.expected.iter().map(|name| path(name)).collect();
    ensure!(
        cycle == expected,
        "cycle {cycle:?} did not match expected {expected:?}"
    );
    Ok(())
}

#[rstest]
#[case::explicit_cycle(CycleCase {
    edges: &[(&["b"], &[], "a"), (&["a"], &[], "b")],
    expected: &["a", "b", "a"],
})]
#[case::implicit_cycle(CycleCase {
    edges: &[(&[], &["b"], "a"), (&[], &["a"], "b")],
    expected: &["a", "b", "a"],
})]
#[case::mixed_cycle(CycleCase {
    edges: &[(&["b"], &[], "a"), (&[], &["c"], "b"), (&["a"], &[], "c")],
    expected: &["a", "b", "c", "a"],
})]
fn find_cycle_identifies_dependency_cycles(#[case] case: CycleCase<'_>) -> Result<()> {
    assert_cycle(&case)
}
#[test]
fn cycle_detector_stack_is_empty_after_cycle_detected() {
    let mut graph = BuildGraph::default();
    graph
        .insert_edge(build_edge(&["b"], &[], "a"))
        .expect("test graph output aliases must be unique");
    graph
        .insert_edge(build_edge(&["a"], &[], "b"))
        .expect("test graph output aliases must be unique");
    let mut detector = CycleDetector::new(&graph);
    assert!(detector.detect().is_some(), "expected a cycle");
    assert!(
        detector.stack.is_empty(),
        "stack must be empty after cycle detection",
    );
}
#[test]
fn bounded_cycles_through_inputs_or_implicit_deps_are_detected() {
    let cases = (2..=5).flat_map(|cycle_len| {
        (0..cycle_len).map(move |implicit_index| (cycle_len, implicit_index))
    });
    for (cycle_len, implicit_index) in cases {
        assert_bounded_cycle_detected(cycle_len, implicit_index);
    }
}
