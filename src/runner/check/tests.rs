//! Snapshot contracts for the `netsuke check` output documents.
//!
//! The JSON documents are a published interface, so they are pinned rather
//! than merely exercised: a field that changes name, moves, or disappears
//! should fail here before it reaches a consumer.

use std::sync::Arc;

use anyhow::{Result, ensure};
use insta::assert_snapshot;
use rstest::rstest;
use test_support::localizer_test_lock;

use crate::ir::BuildGraph;
use crate::lint::{self, Bounds, FailOn, Policy};
use crate::localization::set_localizer_for_tests;
use crate::manifest;
use crate::snapshot_test_support::check_json_snapshot_settings;

use super::{CheckReport, json, text};

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
fn report(limit: usize) -> Result<CheckReport> {
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
    Ok(CheckReport::new(
        "Netsukefile",
        FIXTURE.to_owned(),
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
    let summary =
        test_support::fluent::normalize_fluent_isolates(&text::summary_line(report.report()));
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
    let omitted = report.report().truncated();
    let line =
        test_support::fluent::normalize_fluent_isolates(&text::truncation_line(report.report()));
    ensure!(
        line.contains(&report.report().findings().len().to_string())
            && line.contains(&omitted.to_string()),
        "the notice should state what was shown and what was omitted, got {line}"
    );
    Ok(())
}

/// The shown count selects its CLDR plural variant rather than the default.
///
/// Polish is included because its `few` category (2-4) is what a
/// string-typed argument could never reach; English alone would only tell
/// `one` from `other`.
#[rstest]
#[case::english_one("en-US", 1, "Showing 1 finding;")]
#[case::english_other("en-US", 2, "Showing 2 findings;")]
#[case::polish_one("pl", 1, "Pokazano 1 ustalenie;")]
#[case::polish_few("pl", 2, "Pokazano 2 ustalenia;")]
fn the_truncation_line_agrees_with_the_shown_count(
    #[case] locale: &str,
    #[case] limit: usize,
    #[case] expected: &str,
) -> Result<()> {
    let _lock = localizer_test_lock().map_err(|error| anyhow::anyhow!("{error}"))?;
    let _guard = set_localizer_for_tests(Arc::from(crate::cli_localization::build_localizer(
        Some(locale),
    )));
    let report = report(limit)?;
    ensure!(
        report.report().truncated() > 0,
        "the fixture must exceed a limit of {limit} for the notice to apply"
    );
    let line =
        test_support::fluent::normalize_fluent_isolates(&text::truncation_line(report.report()));
    ensure!(
        line.starts_with(expected),
        "expected {expected:?}, got {line:?}"
    );
    Ok(())
}
