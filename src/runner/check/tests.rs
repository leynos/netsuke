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
    let summary = with_locale("en-US", || {
        let report = report(0)?;
        Ok(test_support::fluent::normalize_fluent_isolates(
            &text::summary_line(report.report()),
        ))
    })?;
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
    with_locale("en-US", || {
        let report = report(1)?;
        let shown = report.report().findings().len();
        let omitted = report.report().truncated();
        let line = test_support::fluent::normalize_fluent_isolates(&text::truncation_line(
            report.report(),
        ));
        ensure!(
            line.contains(&shown.to_string()) && line.contains(&omitted.to_string()),
            "the notice should state what was shown and what was omitted, got {line}"
        );
        Ok(())
    })
}

/// One expected truncation notice for the five-finding fixture.
#[derive(Debug, Clone, Copy)]
struct Notice {
    /// The locale the notice is rendered in.
    locale: &'static str,
    /// The `--limit` applied, which leaves `5 - limit` findings omitted.
    limit: usize,
    /// How the notice must begin, pinning the shown-count clause.
    starts: &'static str,
    /// How the notice must end, pinning the omitted-count clause, where the
    /// case covers it.
    ends: Option<&'static str>,
}

/// Each count in the truncation notice selects its own CLDR plural variant.
///
/// Polish is included because its `few` category (2-4) is what a
/// string-typed argument could never reach; English alone would only tell
/// `one` from `other`. Arabic adds a `two` category, and both of its first two
/// forms express the count in the noun rather than as a numeral. The Italian
/// and Polish omitted clauses select independently of the shown count, so one
/// omitted finding must read as singular whatever was shown.
#[rstest]
#[case::english_one(Notice { locale: "en-US", limit: 1, starts: "Showing 1 finding;", ends: Some("--limit omitted 4 more.") })]
#[case::english_other(Notice { locale: "en-US", limit: 2, starts: "Showing 2 findings;", ends: Some("--limit omitted 3 more.") })]
#[case::polish_one(Notice { locale: "pl", limit: 1, starts: "Pokazano 1 ustalenie;", ends: Some("--limit pominął kolejne 4 ustalenia.") })]
#[case::polish_few(Notice { locale: "pl", limit: 2, starts: "Pokazano 2 ustalenia;", ends: Some("--limit pominął kolejne 3 ustalenia.") })]
#[case::polish_omitted_one(Notice { locale: "pl", limit: 4, starts: "Pokazano 4 ustalenia;", ends: Some("--limit pominął jeszcze 1 ustalenie.") })]
#[case::italian_omitted_one(Notice { locale: "it", limit: 4, starts: "Vengono mostrati 4 rilievi;", ends: Some("--limit ne ha omesso 1 altro.") })]
#[case::italian_omitted_other(Notice { locale: "it", limit: 3, starts: "Vengono mostrati 3 rilievi;", ends: Some("--limit ne ha omessi altri 2.") })]
#[case::arabic_one(Notice { locale: "ar", limit: 1, starts: "يجري عرض نتيجة واحدة؛", ends: None })]
#[case::arabic_two(Notice { locale: "ar", limit: 2, starts: "يجري عرض نتيجتين؛", ends: None })]
fn the_truncation_line_agrees_with_both_counts(#[case] notice: Notice) -> Result<()> {
    let line = with_locale(notice.locale, || truncation_line(notice.limit))?;
    ensure!(
        line.starts_with(notice.starts),
        "expected the notice to start {:?}, got {line:?}",
        notice.starts
    );
    if let Some(ends) = notice.ends {
        ensure!(
            line.ends_with(ends),
            "expected the notice to end {ends:?}, got {line:?}"
        );
    }
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
