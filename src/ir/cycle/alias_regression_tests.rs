//! Regression tests for alias-driven cycle-analysis amplification (#979).
//!
//! Each test fixes one bound the detector must keep: missing records stay
//! proportional to the declared dependency entries, an edge's dependency lists
//! are scanned once rather than once per output alias, and alias witnesses
//! still name the alias that closed the cycle.

use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use rstest::rstest;

use super::analyse;
use super::scan_counter::dependency_scans;
use super::test_fixtures::{build_edge, build_multi_output_edge, path};

use super::super::graph::BuildGraph;

/// Missing dependencies exercised by the alias-amplification regressions.
const EXTERNAL_DEPENDENCIES: [&str; 4] = ["external0", "external1", "external2", "external3"];

/// Build the records one expanded edge must retain for the external entries.
fn expected_external_records(alias: &str) -> Vec<(Utf8PathBuf, Utf8PathBuf)> {
    EXTERNAL_DEPENDENCIES
        .iter()
        .map(|dependency| (path(alias), path(dependency)))
        .collect()
}

/// One multi-output edge whose missing dependencies must be recorded once.
#[test]
fn missing_dependencies_are_recorded_once_for_explicit_aliases() -> Result<()> {
    let mut graph = BuildGraph::default();
    graph
        .insert_edge(build_multi_output_edge(
            &["out0", "out1", "out2", "out3"],
            &[],
            &EXTERNAL_DEPENDENCIES,
            &[],
        ))
        .context("test graph output aliases must be unique")?;

    let report = analyse(&graph);

    ensure!(report.cycle.is_none(), "the generated graph is acyclic");
    ensure!(
        report.missing_dependencies == expected_external_records("out0"),
        "expected one record per missing entry under the entry alias, got {:?}",
        report.missing_dependencies,
    );
    Ok(())
}

/// Implicit output aliases must share the same single-record policy.
#[test]
fn missing_dependencies_are_recorded_once_for_implicit_aliases() -> Result<()> {
    let mut graph = BuildGraph::default();
    graph
        .insert_edge(build_multi_output_edge(
            &["out0"],
            &["out1", "out2", "out3"],
            &[],
            &EXTERNAL_DEPENDENCIES,
        ))
        .context("test graph output aliases must be unique")?;

    let report = analyse(&graph);

    ensure!(report.cycle.is_none(), "the generated graph is acyclic");
    ensure!(
        report.missing_dependencies == expected_external_records("out0"),
        "expected one record per implicit missing entry under the entry alias, got {:?}",
        report.missing_dependencies,
    );
    Ok(())
}

/// Every declared dependency entry of an edge is examined once, not per alias.
#[test]
fn multi_alias_edges_scan_each_dependency_entry_once() -> Result<()> {
    let mut graph = BuildGraph::default();
    graph
        .insert_edge(build_multi_output_edge(
            &["out0", "out1", "out2"],
            &[],
            &["dep0", "dep1"],
            &[],
        ))
        .context("test graph output aliases must be unique")?;
    graph
        .insert_edge(build_edge(&[], &[], "dep0"))
        .context("test graph output aliases must be unique")?;
    graph
        .insert_edge(build_edge(&[], &[], "dep1"))
        .context("test graph output aliases must be unique")?;

    let report = analyse(&graph);

    ensure!(report.cycle.is_none(), "the generated graph is acyclic");
    ensure!(
        dependency_scans() == 2,
        "three aliases over one edge must scan its two entries twice, got {}",
        dependency_scans(),
    );
    Ok(())
}

/// A dependency naming a non-first alias expands the edge under that alias.
#[test]
fn missing_dependency_uses_the_entry_alias_of_the_expanded_edge() -> Result<()> {
    let mut graph = BuildGraph::default();
    graph
        .insert_edge(build_edge(&["z"], &[], "a"))
        .context("test graph output aliases must be unique")?;
    graph
        .insert_edge(build_multi_output_edge(&["b", "z"], &[], &["missing"], &[]))
        .context("test graph output aliases must be unique")?;

    let report = analyse(&graph);

    ensure!(report.cycle.is_none(), "the generated graph is acyclic");
    ensure!(
        report.missing_dependencies == vec![(path("z"), path("missing"))],
        "the record must name the alias that expanded the edge, got {:?}",
        report.missing_dependencies,
    );
    Ok(())
}

/// Reaching a completed edge through a second alias scans nothing again.
#[test]
fn a_visited_edge_reached_through_another_alias_adds_no_scans() -> Result<()> {
    let mut graph = BuildGraph::default();
    graph
        .insert_edge(build_edge(&["o0", "o1"], &[], "consumer"))
        .context("test graph output aliases must be unique")?;
    graph
        .insert_edge(build_multi_output_edge(&["o0", "o1"], &[], &["dep0"], &[]))
        .context("test graph output aliases must be unique")?;
    graph
        .insert_edge(build_edge(&[], &[], "dep0"))
        .context("test graph output aliases must be unique")?;

    let report = analyse(&graph);

    ensure!(report.cycle.is_none(), "the generated graph is acyclic");
    ensure!(
        dependency_scans() == 3,
        "expected consumer's two entries plus the producer's single entry, got {}",
        dependency_scans(),
    );
    Ok(())
}

/// One edge in an alias-witness scenario.
struct AliasEdgeSpec<'a> {
    /// Explicit outputs produced by the edge.
    explicit_outputs: &'a [&'a str],
    /// Implicit outputs produced by the edge.
    implicit_outputs: &'a [&'a str],
    /// Explicit inputs consumed by the edge.
    inputs: &'a [&'a str],
    /// Implicit dependencies consumed by the edge.
    implicit_deps: &'a [&'a str],
}

/// An alias-witness graph and the exact canonical cycle it must report.
struct AliasCycleCase<'a> {
    /// Edges inserted into the test graph.
    edges: &'a [AliasEdgeSpec<'a>],
    /// Expected canonical cycle, with the first node repeated last.
    expected: &'a [&'a str],
}

/// Insert `case`'s edges and require the exact canonical witness.
fn assert_alias_cycle(case: &AliasCycleCase<'_>) -> Result<()> {
    let mut graph = BuildGraph::default();
    for edge in case.edges {
        graph
            .insert_edge(build_multi_output_edge(
                edge.explicit_outputs,
                edge.implicit_outputs,
                edge.inputs,
                edge.implicit_deps,
            ))
            .context("test graph output aliases must be unique")?;
    }
    let cycle = analyse(&graph)
        .cycle
        .context("expected the generated graph to contain a cycle")?;
    let expected: Vec<_> = case.expected.iter().map(|name| path(name)).collect();
    ensure!(
        cycle == expected,
        "cycle {cycle:?} did not match expected {expected:?}",
    );
    Ok(())
}

#[rstest]
#[case::same_edge_alias(AliasCycleCase {
    edges: &[AliasEdgeSpec {
        explicit_outputs: &["a0", "a1"],
        implicit_outputs: &[],
        inputs: &["a1"],
        implicit_deps: &[],
    }],
    expected: &["a1", "a1"],
})]
#[case::non_first_explicit_alias(AliasCycleCase {
    edges: &[
        AliasEdgeSpec {
            explicit_outputs: &["a", "a.extra"],
            implicit_outputs: &[],
            inputs: &["b"],
            implicit_deps: &[],
        },
        AliasEdgeSpec {
            explicit_outputs: &["b"],
            implicit_outputs: &[],
            inputs: &["a.extra"],
            implicit_deps: &[],
        },
    ],
    expected: &["a.extra", "b", "a.extra"],
})]
#[case::implicit_alias(AliasCycleCase {
    edges: &[
        AliasEdgeSpec {
            explicit_outputs: &["a"],
            implicit_outputs: &["a.extra"],
            inputs: &["b"],
            implicit_deps: &[],
        },
        AliasEdgeSpec {
            explicit_outputs: &["b"],
            implicit_outputs: &[],
            inputs: &["a.extra"],
            implicit_deps: &[],
        },
    ],
    expected: &["a.extra", "b", "a.extra"],
})]
fn alias_cycles_keep_exact_witnesses(#[case] case: AliasCycleCase<'_>) -> Result<()> {
    assert_alias_cycle(&case)
}
