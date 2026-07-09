//! Unit tests for cycle detection and canonicalization.
use super::*;
use anyhow::{Context, Result, ensure};
use rstest::rstest;
use std::collections::HashMap;

fn path(name: &str) -> Utf8PathBuf {
    Utf8PathBuf::from(name)
}
fn build_edge(inputs: &[&str], implicit_deps: &[&str], output: &str) -> BuildEdge {
    BuildEdge {
        action_id: "id".into(),
        inputs: inputs.iter().map(|name| path(name)).collect(),
        implicit_deps: implicit_deps.iter().map(|name| path(name)).collect(),
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
    let mut targets = HashMap::new();
    targets.insert(
        path("a"),
        build_edge(case.primary_inputs, case.primary_implicit_deps, "a"),
    );
    for (output, inputs, implicit_deps) in case.extra_targets {
        targets.insert(path(output), build_edge(inputs, implicit_deps, output));
    }
    let expected: Vec<_> = case
        .expected
        .iter()
        .map(|(dependent, missing)| (path(dependent), path(missing)))
        .collect();
    let mut detector = CycleDetector::new(&targets);
    let (target, _) = target_entry_for_path(&targets, path("a").as_path())
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
    targets: &mut HashMap<Utf8PathBuf, BuildEdge>,
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
    targets.insert(output.into(), edge);
}
fn assert_bounded_cycle_detected(cycle_len: usize, implicit_index: usize) {
    let mut targets = HashMap::new();
    for index in 0..cycle_len {
        insert_cycle_edge(&mut targets, index, cycle_len, implicit_index);
    }
    assert!(
        CycleDetector::find_cycle(&targets).is_some(),
        "expected cycle with length {cycle_len} and implicit edge at {implicit_index}",
    );
}
#[test]
fn cycle_detector_detects_self_edge_cycle() {
    let mut targets = HashMap::new();
    targets.insert(path("a"), build_edge(&["a"], &[], "a"));
    let cycle = CycleDetector::find_cycle(&targets).expect("cycle");
    assert_eq!(cycle, vec![path("a"), path("a")]);
}
#[test]
fn cycle_detector_marks_nodes_visited_after_traversal() {
    let mut targets = HashMap::new();
    let a = path("a");
    let b = path("b");
    targets.insert(a.clone(), build_edge(&["b"], &[], "a"));
    targets.insert(b.clone(), build_edge(&[], &[], "b"));
    let mut detector = CycleDetector::new(&targets);
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
#[test]
fn find_cycle_identifies_cycle() {
    let mut targets = HashMap::new();
    targets.insert(path("a"), build_edge(&["b"], &[], "a"));
    targets.insert(path("b"), build_edge(&["a"], &[], "b"));
    let cycle = CycleDetector::find_cycle(&targets).expect("cycle");
    assert_eq!(cycle, vec![path("a"), path("b"), path("a")]);
}
#[test]
fn find_cycle_identifies_implicit_dependency_cycle() {
    let mut targets = HashMap::new();
    targets.insert(path("a"), build_edge(&[], &["b"], "a"));
    targets.insert(path("b"), build_edge(&[], &["a"], "b"));
    let cycle = CycleDetector::find_cycle(&targets).expect("cycle");
    assert_eq!(cycle, vec![path("a"), path("b"), path("a")]);
}
#[test]
fn cycle_detector_stack_is_empty_after_cycle_detected() {
    let mut targets = HashMap::new();
    targets.insert(path("a"), build_edge(&["b"], &[], "a"));
    targets.insert(path("b"), build_edge(&["a"], &[], "b"));
    let mut detector = CycleDetector::new(&targets);
    assert!(detector.detect().is_some(), "expected a cycle");
    assert!(
        detector.stack.is_empty(),
        "stack must be empty after cycle detection",
    );
}
#[test]
fn find_cycle_identifies_mixed_input_and_implicit_dependency_cycle() {
    let mut targets = HashMap::new();
    targets.insert(path("a"), build_edge(&["b"], &[], "a"));
    targets.insert(path("b"), build_edge(&[], &["c"], "b"));
    targets.insert(path("c"), build_edge(&["a"], &[], "c"));
    let cycle = CycleDetector::find_cycle(&targets).expect("cycle");
    assert_eq!(cycle, vec![path("a"), path("b"), path("c"), path("a")]);
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
