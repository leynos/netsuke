//! Serialize Netsuke diagnostics into a stable JSON document.
//!
//! This module owns Netsuke's machine-readable diagnostic schema rather than
//! exposing upstream formatter output directly. The schema is intentionally
//! small, versioned, and derived from `miette` diagnostics when available.

use miette::{Diagnostic, Report};
use serde::Serialize;
use std::error::Error as StdError;
use std::io::{self, Write};
use std::process::ExitCode;

use crate::json_envelope::{GeneratorInfo, SCHEMA_VERSION};

#[path = "diagnostic_json_support.rs"]
mod diagnostic_json_support;
use self::diagnostic_json_support::{
    DiagnosticSource, DiagnosticSpan, collect_diagnostic_causes, collect_error_causes,
    diagnostic_help, diagnostic_url, extract_source_and_labels, fallback_payload, severity_name,
};

/// Render a [`miette::Report`] as Netsuke's JSON diagnostic document.
///
/// # Errors
///
/// Returns an error if the document cannot be serialized to JSON.
pub fn render_report_json(report: &Report) -> serde_json::Result<String> {
    render_diagnostic_json(report.as_ref())
}

/// Render a [`miette::Diagnostic`] as Netsuke's JSON diagnostic document.
///
/// # Errors
///
/// Returns an error if the document cannot be serialized to JSON.
pub fn render_diagnostic_json(diagnostic: &dyn Diagnostic) -> serde_json::Result<String> {
    serde_json::to_string_pretty(&DiagnosticDocument::from_diagnostic(diagnostic))
}

/// Render a plain error as Netsuke's JSON diagnostic document.
///
/// This path is used for startup failures that do not carry `miette`
/// diagnostics, such as clap or configuration-load errors.
///
/// # Errors
///
/// Returns an error if the document cannot be serialized to JSON.
pub fn render_error_json(error: &(dyn StdError + 'static)) -> serde_json::Result<String> {
    serde_json::to_string_pretty(&DiagnosticDocument::from_error(error))
}

/// Emit a rendered JSON diagnostic document to `stderr`, falling back to a
/// minimal schema-compatible payload when serialization fails.
#[must_use]
pub fn emit_or_fallback(result: serde_json::Result<String>) -> ExitCode {
    let payload = result.unwrap_or_else(|err| fallback_payload(&err));
    drop(writeln!(io::stderr(), "{payload}"));
    ExitCode::FAILURE
}

/// The versioned JSON document wrapping one or more diagnostic entries.
#[derive(Debug, Serialize, PartialEq, Eq)]
struct DiagnosticDocument {
    /// Schema version of the diagnostic envelope.
    schema_version: u32,
    /// Generator identity stamped from the build environment.
    generator: GeneratorInfo,
    /// The rendered diagnostic entries.
    diagnostics: Vec<DiagnosticEntry>,
}

impl DiagnosticDocument {
    /// Build a document from a miette diagnostic.
    fn from_diagnostic(diagnostic: &dyn Diagnostic) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            generator: GeneratorInfo::current(),
            diagnostics: vec![DiagnosticEntry::from_diagnostic(diagnostic)],
        }
    }

    /// Build a document from a plain standard error.
    fn from_error(error: &(dyn StdError + 'static)) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            generator: GeneratorInfo::current(),
            diagnostics: vec![DiagnosticEntry::from_error(error)],
        }
    }
}

/// One miette diagnostic rendered into the JSON schema.
#[derive(Debug, Serialize, PartialEq, Eq)]
struct DiagnosticEntry {
    /// The rendered diagnostic message.
    message: String,
    /// The diagnostic's machine-readable code, when it has one.
    code: Option<String>,
    /// The schema's severity name: `error`, `warning`, or `advice`.
    severity: &'static str,
    /// The diagnostic's help text, when provided.
    help: Option<String>,
    /// The documentation URL, when provided.
    url: Option<String>,
    /// The cause chain rendered as strings.
    causes: Vec<String>,
    /// The named source file, when labels are available.
    source: Option<DiagnosticSource>,
    /// The primary labelled span, when the source carries labels.
    primary_span: Option<DiagnosticSpan>,
    /// Every labelled span of the diagnostic.
    labels: Vec<DiagnosticSpan>,
    /// Diagnostics related to this one.
    related: Vec<Self>,
}

impl DiagnosticEntry {
    /// Build an entry from a miette diagnostic, capturing its spans and causes.
    fn from_diagnostic(diagnostic: &dyn Diagnostic) -> Self {
        let (source, primary_span, labels) = extract_source_and_labels(diagnostic);
        let related = diagnostic
            .related()
            .map(|items| items.map(Self::from_diagnostic).collect())
            .unwrap_or_default();
        Self {
            message: diagnostic.to_string(),
            code: diagnostic.code().map(|value| value.to_string()),
            severity: severity_name(diagnostic.severity()),
            help: diagnostic_help(diagnostic),
            url: diagnostic_url(diagnostic),
            causes: collect_diagnostic_causes(diagnostic),
            source,
            primary_span,
            labels,
            related,
        }
    }

    /// Build an entry from a plain standard error, without span data.
    fn from_error(error: &(dyn StdError + 'static)) -> Self {
        Self {
            message: error.to_string(),
            code: None,
            severity: "error",
            help: None,
            url: None,
            causes: collect_error_causes(error),
            source: None,
            primary_span: None,
            labels: Vec::new(),
            related: Vec::new(),
        }
    }
}

#[path = "diagnostic_json_tests.rs"]
#[cfg(test)]
mod tests;
