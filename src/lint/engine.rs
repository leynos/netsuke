//! Stage execution, suppression, and ordering.
//!
//! The engine runs each enabled rule against the artefact its stage owns,
//! stamps the policy-resolved severity onto every finding, lets the directive
//! stage report on the suppression comments themselves, applies suppression,
//! and orders the result deterministically.

use crate::ast::NetsukeManifest;
use crate::ir::BuildGraph;

use super::document::Document;
use super::finding::Finding;
use super::policy::Policy;
use super::registry::{self, Registered};
use super::rule::{DirectiveContext, FindingSink, GraphContext, ManifestContext};
use super::severity::Severity;
use super::suppress::{self, Directive};

/// The artefacts one lint run inspects.
pub struct Analysis<'a> {
    /// The authored source, indexed with spans.
    pub document: &'a Document,
    /// The expanded and rendered manifest.
    pub manifest: &'a NetsukeManifest,
    /// The lowered build graph.
    pub graph: &'a BuildGraph,
}

/// Findings and suppression bookkeeping from one lint run.
#[derive(Debug)]
pub struct Outcome {
    /// Reported findings, ordered for output.
    pub findings: Vec<Finding>,
    /// Number of findings a directive silenced.
    pub suppressed: usize,
}

impl Outcome {
    /// Count the reported findings at `severity`.
    #[must_use]
    pub fn count_at(&self, severity: Severity) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.severity == severity)
            .count()
    }

    /// Report whether any finding reaches `threshold`.
    #[must_use]
    pub fn has_severity_at_least(&self, threshold: Severity) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.severity >= threshold)
    }
}

/// Run every rule `policy` enables over `analysis`.
#[must_use]
pub fn run(analysis: &Analysis<'_>, policy: &Policy) -> Outcome {
    let directives = suppress::collect(analysis.document);
    let mut findings = run_stage_rules(analysis, policy);
    let usage = usage_counts(&findings, &directives);
    findings.extend(run_directive_rules(
        analysis.document,
        policy,
        &directives,
        &usage,
    ));
    let before = findings.len();
    findings.retain(|finding| !is_suppressed(finding, &directives));
    let suppressed = before - findings.len();
    findings.sort_by(Finding::compare);
    Outcome {
        findings,
        suppressed,
    }
}

/// Run each enabled document, manifest, and graph rule.
fn run_stage_rules(analysis: &Analysis<'_>, policy: &Policy) -> Vec<Finding> {
    let mut findings = Vec::new();
    let manifest_ctx = ManifestContext {
        manifest: analysis.manifest,
        document: analysis.document,
    };
    let graph_ctx = GraphContext {
        graph: analysis.graph,
        manifest: analysis.manifest,
        document: analysis.document,
    };
    for entry in registry::all() {
        let Some(mut sink) = bind(policy, &entry, &mut findings) else {
            continue;
        };
        match entry {
            Registered::Document(rule) => rule.check(analysis.document, &mut sink),
            Registered::Manifest(rule) => rule.check(&manifest_ctx, &mut sink),
            Registered::Graph(rule) => rule.check(&graph_ctx, &mut sink),
            Registered::Directive(_) => {}
        }
    }
    findings
}

/// Run each enabled directive rule over the collected directives.
fn run_directive_rules(
    document: &Document,
    policy: &Policy,
    directives: &[Directive],
    usage: &[usize],
) -> Vec<Finding> {
    let ctx = DirectiveContext {
        directives,
        usage,
        document,
    };
    let mut findings = Vec::new();
    for entry in registry::all() {
        let Registered::Directive(rule) = entry else {
            continue;
        };
        let Some(severity) = policy.severity_of(rule.meta().name) else {
            continue;
        };
        let mut sink = FindingSink::new(rule.meta(), severity, &mut findings);
        rule.check(&ctx, &mut sink);
    }
    findings
}

/// Bind a sink to `entry` when policy enables it.
fn bind<'a>(
    policy: &Policy,
    entry: &Registered,
    findings: &'a mut Vec<Finding>,
) -> Option<FindingSink<'a>> {
    let meta = entry.meta();
    let severity = policy.severity_of(meta.name)?;
    Some(FindingSink::new(meta, severity, findings))
}

/// Count how many findings each directive silences.
///
/// Counting happens before suppression is applied, so a directive that did its
/// job is recorded as used even though the finding it silenced never reaches
/// the output.
fn usage_counts(findings: &[Finding], directives: &[Directive]) -> Vec<usize> {
    directives
        .iter()
        .map(|directive| {
            findings
                .iter()
                .filter(|finding| {
                    directive.names(finding.meta.name) && directive.covers(finding.span())
                })
                .count()
        })
        .collect()
}

/// Report whether any directive silences `finding`.
fn is_suppressed(finding: &Finding, directives: &[Directive]) -> bool {
    directives
        .iter()
        .any(|directive| directive.names(finding.meta.name) && directive.covers(finding.span()))
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
