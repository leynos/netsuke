//! Generated multi-alias graphs keep analysis proportional to the manifest.
//!
//! Every generated edge declares two or three explicit aliases and up to two
//! implicit aliases, carries at least one absent dependency, and may gain one
//! forward back arc that closes a cycle. The properties compare the detector's
//! scan count and missing-record count with the declared dependency entries,
//! check that each witness pair is a real dependency of its producer, and
//! compare cycle presence with the independent edge-level oracle.

mod graph;

use camino::Utf8PathBuf;
use proptest::prelude::*;

use self::graph::{
    AliasCounts, BackArcKind, BackArcSpec, build_graph, declared_entries, graph_strategy,
};
use super::super::super::graph::BuildGraph;
use super::super::analyse;
use super::super::scan_counter::dependency_scans;
use super::edge_oracle::edge_graph_has_cycle;

/// Describe every invariant the analysis of `graph` violates.
fn analysis_violations(graph: &BuildGraph) -> Vec<String> {
    let declared = declared_entries(graph);
    let report = analyse(graph);
    let mut violations = Vec::new();

    if report.missing_dependencies.len() > declared {
        violations.push(format!(
            "{} missing records exceed {declared} declared entries",
            report.missing_dependencies.len(),
        ));
    }
    if dependency_scans() > declared {
        violations.push(format!(
            "{} dependency scans exceed {declared} declared entries",
            dependency_scans(),
        ));
    }
    if let Some(cycle) = report.cycle.as_ref() {
        violations.extend(witness_violations(cycle, graph));
    }
    if report.cycle.is_some() != edge_graph_has_cycle(graph) {
        violations.push("cycle presence disagrees with the edge-level oracle".to_owned());
    }
    violations
}

/// Describe every place `cycle` fails to be a real, closed alias path.
fn witness_violations(cycle: &[Utf8PathBuf], graph: &BuildGraph) -> Vec<String> {
    let mut violations = Vec::new();
    if cycle.len() < 2 || cycle.first() != cycle.last() {
        violations.push(format!("cycle {cycle:?} is not closed"));
    }
    for pair in cycle.windows(2) {
        let [left, right] = pair else {
            continue;
        };
        let Some((_, edge)) = graph.target_for_output(left.as_path()) else {
            violations.push(format!("cycle node {left} has no producing edge"));
            continue;
        };
        if !edge.inputs.contains(right) && !edge.implicit_deps.contains(right) {
            violations.push(format!(
                "cycle arc {left} -> {right} is not a declared dependency",
            ));
        }
    }
    violations
}

/// Build a two-edge graph whose only cycle closes through an implicit alias.
fn representative_cyclic_graph() -> BuildGraph {
    let counts = [
        AliasCounts {
            explicit: 2,
            implicit: 1,
        },
        AliasCounts {
            explicit: 2,
            implicit: 0,
        },
    ];
    let extras = [Vec::new(), Vec::new()];
    let back_arc = BackArcSpec {
        kind: BackArcKind::ImplicitOutput,
        from: 0,
        to: 1,
        alias: 0,
    };
    build_graph(&counts, &extras, back_arc)
}

proptest! {
    /// Generated multi-alias graphs keep analysis within the declared bounds
    /// and report only real alias paths.
    #[test]
    fn analysis_stays_within_declared_bounds(
        (alias_counts, extra_deps, back_arc) in graph_strategy()
    ) {
        let graph = build_graph(&alias_counts, &extra_deps, back_arc);
        let violations = analysis_violations(&graph);
        prop_assert!(
            violations.is_empty(),
            "graph violated cycle-analysis invariants: {violations:?}",
        );
    }
}

/// The generated shapes are reachable: an acyclic graph with missing records
/// and a cyclic graph closed through a non-first alias both satisfy the
/// invariants.
#[test]
fn representative_generated_shapes_satisfy_the_invariants() {
    let counts = [
        AliasCounts {
            explicit: 2,
            implicit: 1,
        },
        AliasCounts {
            explicit: 2,
            implicit: 0,
        },
    ];
    let acyclic = build_graph(
        &counts,
        &[Vec::new(), Vec::new()],
        BackArcSpec {
            kind: BackArcKind::None,
            from: 0,
            to: 1,
            alias: 0,
        },
    );
    let cyclic = representative_cyclic_graph();

    let acyclic_violations = analysis_violations(&acyclic);
    assert!(
        acyclic_violations.is_empty(),
        "acyclic representative violated invariants: {acyclic_violations:?}",
    );
    let cyclic_violations = analysis_violations(&cyclic);
    assert!(
        cyclic_violations.is_empty(),
        "cyclic representative violated invariants: {cyclic_violations:?}",
    );
    assert!(
        analyse(&acyclic).cycle.is_none(),
        "the acyclic representative must stay acyclic",
    );
    assert!(
        analyse(&cyclic).cycle.is_some(),
        "the cyclic representative must report a cycle",
    );
    assert!(
        declared_entries(&acyclic) > 0,
        "the missing-record bound must be exercised",
    );
}
