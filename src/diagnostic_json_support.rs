//! Private helpers for [`super`]'s JSON diagnostic document.
//!
//! The span extraction, cause collection, and fallback-payload machinery keeps
//! `diagnostic_json.rs` within the repository's 400-line cap. Nothing here is
//! reachable from outside the diagnostic document module.

use std::error::Error as StdError;
use std::iter::Peekable;
use std::str::Chars;

use miette::{Diagnostic, LabeledSpan, Severity, SourceCode, SourceSpan, SpanContents};
use serde::Serialize;

use crate::json_envelope::{GeneratorInfo, SCHEMA_VERSION};

/// The named source file a diagnostic's primary span points into.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub(super) struct DiagnosticSource {
    /// The source file's display name.
    pub(super) name: String,
}

/// One labelled span within the diagnostic source, ready for serialization.
#[derive(Debug, Serialize, PartialEq, Eq, Clone)]
pub(super) struct DiagnosticSpan {
    /// The label rendered beside the span, when the diagnostic provides one.
    pub(super) label: Option<String>,
    /// Byte offset of the span's start within the source.
    pub(super) offset: usize,
    /// Length of the span in bytes.
    pub(super) length: usize,
    /// One-based line of the span's start.
    pub(super) line: u32,
    /// One-based column of the span's start.
    pub(super) column: u32,
    /// One-based line of the span's end.
    pub(super) end_line: u32,
    /// One-based column of the span's end.
    pub(super) end_column: u32,
    /// First line of the span's text, when it can be decoded.
    pub(super) snippet: Option<String>,
}

/// Map a miette severity to the schema's JSON severity name.
pub(super) fn severity_name(severity: Option<Severity>) -> &'static str {
    match severity.unwrap_or(Severity::Error) {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Advice => "advice",
    }
}

/// Collect the diagnostic-source chain of a miette diagnostic as strings.
pub(super) fn collect_diagnostic_causes(diagnostic: &dyn Diagnostic) -> Vec<String> {
    if let Some(source) = diagnostic.diagnostic_source() {
        return collect_diagnostic_chain(source);
    }
    collect_error_causes_from_option(diagnostic.source())
}

/// Collect the full miette diagnostic-source chain as strings.
fn collect_diagnostic_chain(diagnostic: &dyn Diagnostic) -> Vec<String> {
    let mut causes = vec![diagnostic.to_string()];
    if let Some(source) = diagnostic.diagnostic_source() {
        causes.extend(collect_diagnostic_chain(source));
    } else {
        causes.extend(collect_error_causes_from_option(diagnostic.source()));
    }
    causes
}

/// Collect a standard-error source chain as strings.
pub(super) fn collect_error_causes(error: &(dyn StdError + 'static)) -> Vec<String> {
    collect_error_causes_from_option(error.source())
}

/// Collect every remaining link in a standard-error source chain.
fn collect_error_causes_from_option(mut current: Option<&(dyn StdError + 'static)>) -> Vec<String> {
    let mut causes = Vec::new();
    while let Some(error) = current {
        causes.push(error.to_string());
        current = error.source();
    }
    causes
}

/// Return the source name, primary span, and labelled spans of a diagnostic.
///
/// The search climbs the diagnostic-source chain when the outermost diagnostic
/// carries no labels, matching how miette itself locates the rendered error.
pub(super) fn extract_source_and_labels(
    diagnostic: &dyn Diagnostic,
) -> (
    Option<DiagnosticSource>,
    Option<DiagnosticSpan>,
    Vec<DiagnosticSpan>,
) {
    let Some(labelled_diagnostic) = diagnostic_with_labels(diagnostic) else {
        return (None, None, Vec::new());
    };
    let Some(source_code) = labelled_diagnostic.source_code() else {
        return (None, None, Vec::new());
    };
    let Some(labels) = labelled_diagnostic.labels() else {
        return (None, None, Vec::new());
    };

    let mut source = None;
    let mut primary_span = None;
    let spans = labels
        .filter_map(|label| {
            if source.is_none() {
                source = source_name_for(&label, source_code).map(|name| DiagnosticSource { name });
            }
            let is_primary = label.primary();
            let span = build_span(&label, source_code)?;
            if is_primary {
                primary_span = Some(span.clone());
            }
            Some(span)
        })
        .collect();
    (source, primary_span, spans)
}

/// Build the minimal schema-compatible payload emitted when serialization fails.
///
/// The stderr diagnostic must remain valid JSON even when the document render
/// itself errors, so the fallback hard-codes a `diagnostics` array and only
/// interpolates the generator identity and the serialization error text.
pub(super) fn fallback_payload(error: &serde_json::Error) -> String {
    let document = serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "generator": GeneratorInfo::current(),
        "diagnostics": [{
            "message": "Failed to serialize diagnostics JSON.",
            "code": null,
            "severity": "error",
            "help": null,
            "url": null,
            "causes": [error.to_string()],
            "source": null,
            "primary_span": null,
            "labels": [],
            "related": [],
        }],
    });
    serde_json::to_string_pretty(&document).unwrap_or_else(|_| {
        format!(
            concat!(
                "{{\"schema_version\":{},",
                "\"generator\":{{\"name\":\"{}\",\"version\":\"{}\"}},",
                "\"diagnostics\":[]}}"
            ),
            SCHEMA_VERSION,
            GeneratorInfo::current().name,
            GeneratorInfo::current().version,
        )
    })
}

/// Return the diagnostic's own help text, or the nearest source's.
pub(super) fn diagnostic_help(diagnostic: &dyn Diagnostic) -> Option<String> {
    diagnostic
        .help()
        .map(|value| value.to_string())
        .or_else(|| diagnostic.diagnostic_source().and_then(diagnostic_help))
}

/// Return the diagnostic's own URL, or the nearest source's.
pub(super) fn diagnostic_url(diagnostic: &dyn Diagnostic) -> Option<String> {
    diagnostic
        .url()
        .map(|value| value.to_string())
        .or_else(|| diagnostic.diagnostic_source().and_then(diagnostic_url))
}

/// Find the outermost diagnostic in the chain that still carries labels.
fn diagnostic_with_labels(diagnostic: &dyn Diagnostic) -> Option<&dyn Diagnostic> {
    if diagnostic.source_code().is_some() && diagnostic.labels().is_some() {
        Some(diagnostic)
    } else {
        diagnostic
            .diagnostic_source()
            .and_then(diagnostic_with_labels)
    }
}

/// Return the source name a label points into, when the span has one.
fn source_name_for(label: &LabeledSpan, source_code: &dyn SourceCode) -> Option<String> {
    let contents = source_code.read_span(label.inner(), 0, 0).ok()?;
    contents.name().map(ToOwned::to_owned)
}

/// Build a serializable span from a miette label and its source contents.
fn build_span(label: &LabeledSpan, source_code: &dyn SourceCode) -> Option<DiagnosticSpan> {
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

/// Return the first line of a span's snippet, with the CRLF terminator removed.
fn span_snippet(contents: &dyn SpanContents<'_>) -> Option<String> {
    let data = std::str::from_utf8(contents.data()).ok()?;
    let first_line = data.lines().next()?.trim_end_matches('\r');
    Some(first_line.to_owned())
}

/// Return the 1-based (start line, column) and (end line, column) positions.
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

/// Return the exact text a span covers within the contents.
fn exact_span_text(contents: &dyn SpanContents<'_>, span: &SourceSpan) -> Option<String> {
    let data = std::str::from_utf8(contents.data()).ok()?;
    let start = byte_index_for_column(data, contents.column())?;
    let end = start.checked_add(span.len())?;
    data.get(start..end).map(ToOwned::to_owned)
}

/// Return the byte index of a column within a one-line text.
fn byte_index_for_column(text: &str, column: usize) -> Option<usize> {
    let line_end = text.find('\n').unwrap_or(text.len());
    let line = text.get(..line_end)?;
    Some(
        line.char_indices()
            .nth(column)
            .map_or(line.len(), |(index, _)| index),
    )
}

/// Return whether the char starts a CRLF pair, consuming it from `chars`.
fn should_skip_crlf(current: char, chars: &mut Peekable<Chars<'_>>) -> bool {
    current == '\r' && chars.peek().is_some_and(|next| *next == '\n')
}

/// Advance the walking position over `text`, counting lines and columns.
fn end_position(start_line: usize, start_column: usize, text: &str) -> (u32, u32) {
    let mut line = start_line;
    let mut column = start_column;
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if should_skip_crlf(ch, &mut chars) {
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

/// Convert a `usize` to `u32`, saturating at the maximum value.
fn to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}
