//! Human rendering for a passing `netsuke check`.
//!
//! A failing check renders through the threshold diagnostic, which `miette`
//! prints with source snippets. This module covers the passing case, where
//! findings below the threshold still need reporting, and the clean case,
//! where the summary is the whole output.

use miette::Report as MietteReport;

use crate::cli::Cli;
use crate::lint::{Report, Severity};
use crate::localization::{self, keys};

use super::super::check_diagnostics::CheckReport;

/// Render a passing check for a human reader.
#[must_use]
pub(super) fn render(check: &CheckReport, cli: &Cli) -> String {
    let report = check.report();
    let mut rendered = String::new();
    if !cli.json {
        for diagnostic in check.diagnostics() {
            let printed = MietteReport::new(diagnostic);
            push_line(&mut rendered, &format!("{printed:?}"));
        }
    }
    push_line(&mut rendered, &summary_line(report));
    if report.truncated() > 0 {
        push_line(&mut rendered, &truncation_line(report));
    }
    rendered
}

/// Append one line to the rendered report.
fn push_line(rendered: &mut String, line: &str) {
    rendered.push_str(line);
    rendered.push('\n');
}

/// Build the localized summary line for a report.
#[must_use]
pub(super) fn summary_line(report: &Report) -> String {
    if report.findings().is_empty() {
        return localization::message(keys::CHECK_SUMMARY_CLEAN).to_string();
    }
    localization::message(keys::CHECK_SUMMARY_COUNTS)
        .with_arg("errors", report.count_at(Severity::Error).to_string())
        .with_arg("warnings", report.count_at(Severity::Warning).to_string())
        .with_arg("advice", report.count_at(Severity::Advice).to_string())
        .with_arg("suppressed", report.suppressed().to_string())
        .to_string()
}

/// Build the localized truncation notice for a report.
#[must_use]
pub(super) fn truncation_line(report: &Report) -> String {
    localization::message(keys::CHECK_SUMMARY_TRUNCATED)
        .with_arg("shown", report.findings().len().to_string())
        .with_arg("omitted", report.truncated().to_string())
        .to_string()
}
