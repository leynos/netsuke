//! JSON result documents for `netsuke check`.
//!
//! A successful check emits a result document whose `findings` array holds the
//! same entries the failure branch carries under `diagnostics[0].related`, so a
//! consumer parses one finding shape and selects the array by presence.

use anyhow::Result;
use serde::Serialize;

use crate::diagnostic_json::{DiagnosticEntry, diagnostic_entry};
use crate::json_envelope::{GeneratorInfo, SCHEMA_VERSION};
use crate::lint::{Report, RuleMeta, Severity};

use super::super::check_diagnostics::CheckReport;
use super::super::check_documentation::rule_documentation_url;

/// Render a passing check as its versioned result document.
///
/// # Errors
///
/// Returns an error when the document cannot be serialized to JSON.
pub(super) fn render_result(check: &CheckReport) -> Result<String> {
    let report = check.report();
    let findings = check
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic_entry(diagnostic))
        .collect();
    let document = CheckDocument {
        schema_version: SCHEMA_VERSION,
        generator: GeneratorInfo::current(),
        result: CheckResult {
            command: "check",
            status: "pass",
            fail_on: report.threshold().as_str(),
            summary: Summary::new(report),
            truncated: report.truncated() > 0,
            findings,
        },
    };
    Ok(serde_json::to_string_pretty(&document)?)
}

/// Render the rule catalogue as its versioned result document.
///
/// # Errors
///
/// Returns an error when the document cannot be serialized to JSON.
pub(super) fn render_catalogue(rules: &[&'static RuleMeta]) -> Result<String> {
    let document = ExplainDocument {
        schema_version: SCHEMA_VERSION,
        generator: GeneratorInfo::current(),
        result: ExplainResult {
            command: "check-explain",
            rules: rules.iter().copied().map(RuleEntry::new).collect(),
        },
    };
    Ok(serde_json::to_string_pretty(&document)?)
}

/// The versioned document wrapping a passing check.
#[derive(Debug, Serialize)]
struct CheckDocument {
    /// Schema version of the document envelope.
    schema_version: u32,
    /// Generator identity stamped from the build environment.
    generator: GeneratorInfo,
    /// The check's outcome.
    result: CheckResult,
}

/// The outcome of a passing check.
#[derive(Debug, Serialize)]
struct CheckResult {
    /// The command that produced the result.
    command: &'static str,
    /// Always `pass`: a failing check emits a diagnostic document instead.
    status: &'static str,
    /// The failure threshold the run was measured against.
    fail_on: &'static str,
    /// Finding counts by severity.
    summary: Summary,
    /// Whether `--limit` excluded any finding.
    truncated: bool,
    /// Every reported finding, in output order.
    findings: Vec<DiagnosticEntry>,
}

/// Finding counts for one check.
///
/// The severity tallies describe the whole run, while `reported` and `omitted`
/// describe the bounded output, so `error + warning + advice` always equals
/// `reported + omitted`. Counting the tallies before the limit is what lets a
/// consumer see that a run found an error even when `--limit` kept it out of
/// `findings`.
#[derive(Debug, Serialize)]
struct Summary {
    /// Findings at error severity across the whole run.
    error: usize,
    /// Findings at warning severity across the whole run.
    warning: usize,
    /// Findings at advice severity across the whole run.
    advice: usize,
    /// Findings present in `findings`, after `--limit` applies.
    reported: usize,
    /// Findings a directive silenced, and so never counted above.
    suppressed: usize,
    /// Findings `--limit` excluded from `findings`.
    omitted: usize,
}

impl Summary {
    /// Summarize a report's findings.
    fn new(report: &Report) -> Self {
        Self {
            error: report.count_at(Severity::Error),
            warning: report.count_at(Severity::Warning),
            advice: report.count_at(Severity::Advice),
            reported: report.findings().len(),
            suppressed: report.suppressed(),
            omitted: report.truncated(),
        }
    }
}

/// The versioned document wrapping the rule catalogue.
#[derive(Debug, Serialize)]
struct ExplainDocument {
    /// Schema version of the document envelope.
    schema_version: u32,
    /// Generator identity stamped from the build environment.
    generator: GeneratorInfo,
    /// The catalogue.
    result: ExplainResult,
}

/// The rule catalogue an editor or agent reads to build a rule picker.
#[derive(Debug, Serialize)]
struct ExplainResult {
    /// The command that produced the result.
    command: &'static str,
    /// Every rule requested, ordered by category then name.
    rules: Vec<RuleEntry>,
}

/// One rule's published metadata.
#[derive(Debug, Serialize)]
struct RuleEntry {
    /// The rule's stable identifier.
    name: &'static str,
    /// The concern the rule addresses.
    category: &'static str,
    /// The compiler artefact the rule inspects.
    stage: &'static str,
    /// The severity the rule reports at unless policy overrides it.
    default_severity: &'static str,
    /// The rule's diagnostic code.
    code: String,
    /// One-line description of what the rule detects.
    summary: &'static str,
    /// Why the detected construct is a problem.
    rationale: &'static str,
    /// The canonical alternative.
    remediation: &'static str,
    /// Where the rule is documented.
    url: String,
}

impl RuleEntry {
    /// Publish one rule's metadata.
    fn new(meta: &'static RuleMeta) -> Self {
        Self {
            name: meta.name,
            category: meta.category.as_str(),
            stage: meta.stage.as_str(),
            default_severity: meta.default_severity.as_str(),
            code: meta.code(),
            summary: meta.summary,
            rationale: meta.rationale,
            remediation: meta.remediation,
            url: rule_documentation_url(meta.name),
        }
    }
}
