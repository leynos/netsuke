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

use crate::cli::{CheckArgs, Cli};
use crate::ir::BuildGraph;
use crate::lint::{self, Bounds, FailOn, Policy};
use crate::localization::set_localizer_for_tests;
use crate::manifest;
use crate::snapshot_test_support::check_json_snapshot_settings;
use crate::status::SilentReporter;

use super::super::error::RunnerError;
use super::telemetry::CheckFailure;
use super::{CheckReport, finish, handle_check_inner, json, text};

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

/// A manifest whose only finding is silenced by a directive.
const SUPPRESSED_ONLY: &str = concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  # netsuke-lint: allow background-job -- deliberate\n",
    "  - name: out\n",
    "    command: \"feh preview &\"\n",
);

/// Lint `FIXTURE` and bound the result to `limit`.
fn report(limit: usize) -> Result<CheckReport> {
    lint_report(FIXTURE, limit, FailOn::Error)
}

/// Lint `source` and bound the result to `limit` and `threshold`.
fn lint_report(source: &str, limit: usize, threshold: FailOn) -> Result<CheckReport> {
    let parsed = manifest::from_str(source)?;
    let graph = BuildGraph::from_manifest(&parsed)?;
    let outcome = lint::analyse(
        lint::Request {
            source: source.to_owned(),
            manifest: &parsed,
            graph: &graph,
        },
        &Policy::defaults(),
    )
    .map_err(|failure| anyhow::anyhow!("fixture should index: {}", failure.message))?;
    Ok(CheckReport::new(
        "Netsukefile",
        source.to_owned(),
        outcome,
        Bounds { limit, threshold },
    ))
}

/// Run `check` under `locale`, holding the shared localizer for its duration.
fn with_locale<T>(locale: &str, check: impl FnOnce() -> Result<T>) -> Result<T> {
    let _lock = localizer_test_lock().map_err(|error| anyhow::anyhow!("{error}"))?;
    let _guard = set_localizer_for_tests(Arc::from(crate::cli_localization::build_localizer(
        Some(locale),
    )));
    check()
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
/// `one` from `other`. Arabic adds a `two` category, and both of its first two
/// forms express the count in the noun rather than as a numeral.
#[rstest]
#[case::english_one("en-US", 1, "Showing 1 finding;")]
#[case::english_other("en-US", 2, "Showing 2 findings;")]
#[case::polish_one("pl", 1, "Pokazano 1 ustalenie;")]
#[case::polish_few("pl", 2, "Pokazano 2 ustalenia;")]
#[case::arabic_one("ar", 1, "يجري عرض نتيجة واحدة؛")]
#[case::arabic_two("ar", 2, "يجري عرض نتيجتين؛")]
fn the_truncation_line_agrees_with_the_shown_count(
    #[case] locale: &str,
    #[case] limit: usize,
    #[case] expected: &str,
) -> Result<()> {
    let line = with_locale(locale, || truncation_line(limit))?;
    ensure!(
        line.starts_with(expected),
        "expected {expected:?}, got {line:?}"
    );
    Ok(())
}

/// The omitted count selects its own plural variant, independently of the
/// shown count: the fixture's five findings leave one omitted at a limit of
/// four and two omitted at a limit of three.
#[rstest]
#[case::italian_one("it", 4, "--limit ne ha omesso 1 altro.")]
#[case::italian_other("it", 3, "--limit ne ha omessi altri 2.")]
#[case::polish_one("pl", 4, "--limit pominął jeszcze 1 ustalenie.")]
#[case::polish_few("pl", 3, "--limit pominął kolejne 2 ustalenia.")]
fn the_truncation_line_agrees_with_the_omitted_count(
    #[case] locale: &str,
    #[case] limit: usize,
    #[case] expected: &str,
) -> Result<()> {
    let line = with_locale(locale, || truncation_line(limit))?;
    ensure!(
        line.ends_with(expected),
        "expected {expected:?}, got {line:?}"
    );
    Ok(())
}

/// Render the truncation notice for `FIXTURE` bounded to `limit`.
fn truncation_line(limit: usize) -> Result<String> {
    let report = report(limit)?;
    ensure!(
        report.report().truncated() > 0,
        "the fixture must exceed a limit of {limit} for the notice to apply"
    );
    Ok(test_support::fluent::normalize_fluent_isolates(
        &text::truncation_line(report.report()),
    ))
}

/// A run whose every finding was suppressed is not clean: the summary states
/// the suppressed count, as the JSON summary does.
#[test]
fn a_fully_suppressed_run_still_states_its_suppressed_count() -> Result<()> {
    let summary = with_locale("en-US", || {
        let report = lint_report(SUPPRESSED_ONLY, 0, FailOn::Error)?;
        ensure!(
            report.report().findings().is_empty() && report.report().suppressed() == 1,
            "the fixture should report nothing and suppress one finding"
        );
        Ok(test_support::fluent::normalize_fluent_isolates(
            &text::summary_line(report.report()),
        ))
    })?;
    ensure!(
        summary.contains("suppressed: 1"),
        "the summary should state the suppressed finding, got {summary}"
    );
    Ok(())
}

/// The threshold diagnostic counts both figures over the whole run, so a run
/// truncated by `--limit` cannot report more failing findings than findings.
#[test]
fn the_threshold_message_counts_the_whole_run_under_a_limit() -> Result<()> {
    let (message, total) = with_locale("en-US", || {
        let report = lint_report(FIXTURE, 1, FailOn::Advice)?;
        let domain = report.report();
        let total = domain.findings().len() + domain.truncated();
        ensure!(
            domain.failing_count() == total && total > domain.findings().len(),
            "every finding should fail at an advice threshold, and a limit of one should truncate"
        );
        let error = finish(&report)
            .err()
            .ok_or_else(|| anyhow::anyhow!("an advice threshold should fail the fixture"))?;
        match error.downcast_ref::<RunnerError>() {
            Some(RunnerError::LintThresholdExceeded { message, .. }) => Ok((
                test_support::fluent::normalize_fluent_isolates(&message.to_string()),
                total,
            )),
            other => Err(anyhow::anyhow!(
                "expected a threshold failure, got {other:?}"
            )),
        }
    })?;
    ensure!(
        message.contains(&format!("{total} of {total} reported")),
        "expected both counts over the whole run, got {message}"
    );
    Ok(())
}

/// An unknown `--explain` rule is a policy failure: nothing failed to write,
/// so it must not be recorded as an output failure.
#[test]
fn an_unknown_explain_rule_is_a_policy_failure() {
    let args = CheckArgs {
        explain: Some("no-such-rule".to_owned()),
        ..CheckArgs::default()
    };
    let result = handle_check_inner(&Cli::default(), &args, &SilentReporter);
    assert!(
        matches!(result, Err(CheckFailure::Policy(_))),
        "an unknown rule should be classified as a policy failure"
    );
}
