//! Snapshot contracts for the `netsuke check` output documents.
//!
//! The JSON documents are a published interface, so they are pinned rather
//! than merely exercised: a field that changes name, moves, or disappears
//! should fail here before it reaches a consumer.

use anyhow::{Result, ensure};
use insta::assert_snapshot;

use crate::ir::BuildGraph;
use crate::lint::{self, Bounds, FailOn, NamedManifest, Policy, Report};
use crate::manifest;
use crate::snapshot_test_support::check_json_snapshot_settings;

use super::{json, text};

/// A manifest that reports one finding at each severity the defaults use.
const FIXTURE: &str = concat!(
    "netsuke_version: \"1.0.0\"\n",
    "vars:\n",
    "  spare: unused\n",
    "actions:\n",
    "  - name: clean\n",
    "    command: \"rm -rf build\"\n",
    "targets:\n",
    "  - name: out.txt\n",
    "    command: \"cp $$SRC out.txt\"\n",
);

/// Lint `FIXTURE` and bound the result to `limit`.
fn report(limit: usize) -> Result<Report> {
    let parsed = manifest::from_str(FIXTURE)?;
    let graph = BuildGraph::from_manifest(&parsed)?;
    let outcome = lint::analyse(
        lint::Request {
            source: FIXTURE.to_owned(),
            manifest: &parsed,
            graph: &graph,
        },
        &Policy::defaults(),
    )
    .map_err(|failure| anyhow::anyhow!("fixture should index: {}", failure.message))?;
    Ok(Report::new(
        NamedManifest {
            name: "Netsukefile",
            source: FIXTURE.to_owned(),
        },
        outcome,
        Bounds {
            limit,
            threshold: FailOn::Error,
        },
    ))
}

#[test]
fn the_result_document_shape_is_pinned() -> Result<()> {
    let rendered = json::render_result(&report(0)?)?;
    check_json_snapshot_settings().bind(|| {
        assert_snapshot!("result_document", rendered);
    });
    Ok(())
}

/// A bounded run must say so in both the flag and the summary, so a consumer
/// can tell truncation from a clean tail.
#[test]
fn a_truncated_result_document_reports_what_it_dropped() -> Result<()> {
    let rendered = json::render_result(&report(1)?)?;
    check_json_snapshot_settings().bind(|| {
        assert_snapshot!("truncated_result_document", rendered);
    });
    Ok(())
}

#[test]
fn the_rule_catalogue_shape_is_pinned() -> Result<()> {
    let rendered = json::render_catalogue(&lint::catalogue())?;
    check_json_snapshot_settings().bind(|| {
        assert_snapshot!("rule_catalogue", rendered);
    });
    Ok(())
}

#[test]
fn the_summary_line_states_every_count() -> Result<()> {
    let report = report(0)?;
    let summary = test_support::fluent::normalize_fluent_isolates(&text::summary_line(&report));
    for expected in ["errors", "warnings", "advice", "suppressed"] {
        ensure!(
            summary.contains(expected),
            "the summary should state {expected}, got {summary}"
        );
    }
    Ok(())
}

#[test]
fn the_truncation_line_states_both_counts() -> Result<()> {
    let report = report(1)?;
    let omitted = report.truncated();
    let line = test_support::fluent::normalize_fluent_isolates(&text::truncation_line(&report));
    ensure!(
        line.contains(&report.findings().len().to_string()) && line.contains(&omitted.to_string()),
        "the notice should state what was shown and what was omitted, got {line}"
    );
    Ok(())
}
