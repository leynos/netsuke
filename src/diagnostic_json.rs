//! Serialize Netsuke diagnostics into a stable JSON document.
//!
//! This module owns Netsuke's machine-readable diagnostic schema rather than
//! exposing upstream formatter output directly. The schema is intentionally
//! small, versioned, and derived from `miette` diagnostics when available.

use miette::{Diagnostic, Report, Severity, SourceCode, SourceSpan, SpanContents};
use serde::Serialize;
use std::error::Error as StdError;

const SCHEMA_VERSION: u32 = 1;

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

#[derive(Debug, Serialize, PartialEq, Eq)]
struct DiagnosticDocument {
    schema_version: u32,
    generator: GeneratorInfo,
    diagnostics: Vec<DiagnosticEntry>,
}

impl DiagnosticDocument {
    fn from_diagnostic(diagnostic: &dyn Diagnostic) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            generator: GeneratorInfo::current(),
            diagnostics: vec![DiagnosticEntry::from_diagnostic(diagnostic)],
        }
    }

    fn from_error(error: &(dyn StdError + 'static)) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            generator: GeneratorInfo::current(),
            diagnostics: vec![DiagnosticEntry::from_error(error)],
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct GeneratorInfo {
    name: &'static str,
    version: &'static str,
}

impl GeneratorInfo {
    const fn current() -> Self {
        Self {
            name: "netsuke",
            version: env!("CARGO_PKG_VERSION"),
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct DiagnosticEntry {
    message: String,
    code: Option<String>,
    severity: &'static str,
    help: Option<String>,
    url: Option<String>,
    causes: Vec<String>,
    source: Option<DiagnosticSource>,
    primary_span: Option<DiagnosticSpan>,
    labels: Vec<DiagnosticSpan>,
    related: Vec<Self>,
}

impl DiagnosticEntry {
    fn from_diagnostic(diagnostic: &dyn Diagnostic) -> Self {
        let (source, labels) = extract_source_and_labels(diagnostic);
        let primary_span = labels.first().cloned();
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

#[derive(Debug, Serialize, PartialEq, Eq)]
struct DiagnosticSource {
    name: String,
}

#[derive(Debug, Serialize, PartialEq, Eq, Clone)]
struct DiagnosticSpan {
    label: Option<String>,
    offset: usize,
    length: usize,
    line: u32,
    column: u32,
    end_line: u32,
    end_column: u32,
    snippet: Option<String>,
}

fn severity_name(severity: Option<Severity>) -> &'static str {
    match severity.unwrap_or(Severity::Error) {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Advice => "advice",
    }
}

fn collect_diagnostic_causes(diagnostic: &dyn Diagnostic) -> Vec<String> {
    if let Some(source) = diagnostic.diagnostic_source() {
        return collect_diagnostic_chain(source);
    }
    collect_error_causes_from_option(diagnostic.source())
}

fn collect_diagnostic_chain(diagnostic: &dyn Diagnostic) -> Vec<String> {
    let mut causes = vec![diagnostic.to_string()];
    if let Some(source) = diagnostic.diagnostic_source() {
        causes.extend(collect_diagnostic_chain(source));
    } else {
        causes.extend(collect_error_causes_from_option(diagnostic.source()));
    }
    causes
}

fn collect_error_causes(error: &(dyn StdError + 'static)) -> Vec<String> {
    collect_error_causes_from_option(error.source())
}

fn collect_error_causes_from_option(mut current: Option<&(dyn StdError + 'static)>) -> Vec<String> {
    let mut causes = Vec::new();
    while let Some(error) = current {
        causes.push(error.to_string());
        current = error.source();
    }
    causes
}

fn extract_source_and_labels(
    diagnostic: &dyn Diagnostic,
) -> (Option<DiagnosticSource>, Vec<DiagnosticSpan>) {
    let Some(labelled_diagnostic) = diagnostic_with_labels(diagnostic) else {
        return (None, Vec::new());
    };
    let Some(source_code) = labelled_diagnostic.source_code() else {
        return (None, Vec::new());
    };
    let Some(labels) = labelled_diagnostic.labels() else {
        return (None, Vec::new());
    };

    let mut source = None;
    let spans = labels
        .filter_map(|label| {
            let span = build_span(&label, source_code)?;
            if source.is_none() {
                source = span
                    .snippet
                    .as_ref()
                    .and_then(|_| source_name_for(&label, source_code))
                    .map(|name| DiagnosticSource { name });
            }
            Some(span)
        })
        .collect();
    (source, spans)
}

fn diagnostic_help(diagnostic: &dyn Diagnostic) -> Option<String> {
    diagnostic
        .help()
        .map(|value| value.to_string())
        .or_else(|| diagnostic.diagnostic_source().and_then(diagnostic_help))
}

fn diagnostic_url(diagnostic: &dyn Diagnostic) -> Option<String> {
    diagnostic
        .url()
        .map(|value| value.to_string())
        .or_else(|| diagnostic.diagnostic_source().and_then(diagnostic_url))
}

fn diagnostic_with_labels(diagnostic: &dyn Diagnostic) -> Option<&dyn Diagnostic> {
    if diagnostic.source_code().is_some() && diagnostic.labels().is_some() {
        Some(diagnostic)
    } else {
        diagnostic
            .diagnostic_source()
            .and_then(diagnostic_with_labels)
    }
}

fn source_name_for(label: &miette::LabeledSpan, source_code: &dyn SourceCode) -> Option<String> {
    let contents = source_code.read_span(label.inner(), 0, 0).ok()?;
    contents.name().map(ToOwned::to_owned)
}

fn build_span(label: &miette::LabeledSpan, source_code: &dyn SourceCode) -> Option<DiagnosticSpan> {
    let contents = source_code.read_span(label.inner(), 0, 0).ok()?;
    let snippet = span_snippet(contents.as_ref());
    let (line, column, end_line, end_column) = span_position(contents.as_ref(), label.inner());
    Some(DiagnosticSpan {
        label: label.label().map(ToOwned::to_owned),
        offset: label.offset(),
        length: label.len(),
        line,
        column,
        end_line,
        end_column,
        snippet,
    })
}

fn span_snippet(contents: &dyn SpanContents<'_>) -> Option<String> {
    let data = std::str::from_utf8(contents.data()).ok()?;
    let first_line = data.lines().next()?.trim_end_matches('\r');
    Some(first_line.to_owned())
}

fn span_position(contents: &dyn SpanContents<'_>, span: &SourceSpan) -> (u32, u32, u32, u32) {
    let start_line = contents.line();
    let start_column = contents.column();
    let line = to_u32(start_line + 1);
    let column = to_u32(start_column + 1);

    let Some(exact_span) = exact_span_text(contents, span) else {
        return (line, column, line, column);
    };
    let (end_line, end_column) = end_position(start_line, start_column, &exact_span);
    (line, column, end_line, end_column)
}

fn exact_span_text(contents: &dyn SpanContents<'_>, span: &SourceSpan) -> Option<String> {
    let data = std::str::from_utf8(contents.data()).ok()?;
    let start = byte_index_for_column(data, contents.column())?;
    let end = start.checked_add(span.len())?;
    data.get(start..end).map(ToOwned::to_owned)
}

fn byte_index_for_column(text: &str, column: usize) -> Option<usize> {
    let line_end = text.find('\n').unwrap_or(text.len());
    let line = text.get(..line_end)?;
    Some(
        line.char_indices()
            .nth(column)
            .map_or(line.len(), |(index, _)| index),
    )
}

fn end_position(start_line: usize, start_column: usize, text: &str) -> (u32, u32) {
    let mut line = start_line;
    let mut column = start_column;
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\r' && chars.peek().is_some_and(|next| *next == '\n') {
            continue;
        }
        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }

    (to_u32(line + 1), to_u32(column + 1))
}

fn to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[path = "diagnostic_json_tests.rs"]
#[cfg(test)]
mod tests;
