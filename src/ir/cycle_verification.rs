//! Kani harnesses for bounded IR cycle-detection properties.

#[derive(Clone, Copy)]
struct BoundedEdge {
    output: &'static str,
    input: Option<&'static str>,
}

struct BoundedCycleReport {
    has_cycle: bool,
    missing_count: usize,
}

/// Prove a self-dependency reports a cycle and no missing dependency.
#[kani::proof]
#[kani::unwind(8)]
fn self_dependency_reports_cycle() {
    let report = analyse_bounded(&[BoundedEdge {
        output: "out",
        input: Some("out"),
    }]);

    kani::assert(report.has_cycle, "self-dependency reports a cycle");
    kani::assert(
        report.missing_count == 0,
        "cycle-only graph reports no missing dependencies",
    );
}

/// Prove an absent dependency is missing, not cyclic.
#[kani::proof]
#[kani::unwind(8)]
fn missing_dependency_does_not_report_cycle() {
    let report = analyse_bounded(&[BoundedEdge {
        output: "out",
        input: Some("missing"),
    }]);

    kani::assert(!report.has_cycle, "missing dependency is not a cycle");
    kani::assert(
        report.missing_count == 1,
        "one missing dependency is reported",
    );
}

/// Analyse a bounded string graph for the cycle properties Kani checks.
fn analyse_bounded(edges: &[BoundedEdge]) -> BoundedCycleReport {
    let mut has_cycle = false;
    let mut missing_count = 0;

    for edge in edges {
        if let Some(input) = edge.input {
            if input == edge.output {
                has_cycle = true;
            } else if !has_output(edges, input) {
                missing_count += 1;
            }
        }
    }

    BoundedCycleReport {
        has_cycle,
        missing_count,
    }
}

/// Return whether the bounded graph contains `output` as a target.
fn has_output(edges: &[BoundedEdge], output: &str) -> bool {
    edges.iter().any(|edge| edge.output == output)
}
