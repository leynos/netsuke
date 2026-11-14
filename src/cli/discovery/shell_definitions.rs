//! Validate each trusted `shells` occurrence before composition.
//!
//! A single configuration file is checked as soon as it is read, so a malformed
//! definition is reported against the layer that declared it rather than
//! against an anonymous merged value. Reporting happens during discovery, which
//! runs before any manifest is read, so no definition error can be overtaken by
//! manifest compilation.
//!
//! Each field is validated by the same deserializer the merged schema uses, so
//! one occurrence and the merged registry cannot disagree. The JSON shape is
//! inspected before deserializing, which keeps a wrong-typed value from
//! appearing in a diagnostic: a mistyped `args` may be an arbitrary string, and
//! echoing it would break the rule that a diagnostic never prints an argument
//! vector.
//!
//! Every diagnostic also names the file that declared the occurrence, because
//! the same key and index appear in several layers and the operator needs to
//! know which file to edit. The path is rendered through the same bounding and
//! escaping as a rejected value, so an unusual directory name cannot break the
//! message across lines or emit terminal control sequences.

use camino::Utf8Path;
use ortho_config::OrthoError;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::sync::Arc;

use crate::cli::config::shells::{
    ShellArguments, ShellExecutable,
    preview::{bounded, preview},
};
use crate::shell_selection::ShellName;

use super::super::constants::SHELLS_KEY;
use super::super::validation::validation_error;

/// Name every field a shell definition may declare.
///
/// A field outside this list is rejected rather than ignored, so a misspelt
/// key cannot silently drop the program or arguments the operator intended.
const DEFINITION_FIELDS: [&str; 3] = ["name", "executable", "args"];

/// Test whether a layer declares the `shells` key at all.
///
/// Authority is decided on presence alone, before any typed merging, so an
/// empty or malformed declaration cannot slip past the rule.
pub(super) fn declares_shells(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|fields| fields.contains_key(SHELLS_KEY))
}

/// Explain why a source other than an operator root may not declare definitions.
///
/// The message names the source when the layer has a file, so an operator can
/// tell which untrusted file carried the key.
pub(super) fn untrusted_shells_error(source: Option<&Utf8Path>) -> Arc<OrthoError> {
    OccurrenceReport::new(source).error(SHELLS_KEY, UNTRUSTED_SHELLS_MESSAGE)
}

/// State the trust boundary that reserves `shells` for operator configuration.
const UNTRUSTED_SHELLS_MESSAGE: &str = concat!(
    "the `shells` key may only be declared in an operator configuration file; ",
    "project, explicit, and environment sources are not trusted to declare it"
);

/// Validate every occurrence of `shells` in one trusted file layer.
///
/// An absent key yields no errors, because the field defaults to an empty
/// registry. Every fault is collected rather than thrown, so an operator sees
/// all of a file's mistakes in one run.
pub(super) fn occurrence_errors(value: &Value, source: Option<&Utf8Path>) -> Vec<Arc<OrthoError>> {
    let mut report = OccurrenceReport::new(source);
    let Some(declared) = value.get(SHELLS_KEY) else {
        return report.errors;
    };
    let Some(entries) = declared.as_array() else {
        report.reject(SHELLS_KEY, "must be a list of shell definitions");
        return report.errors;
    };
    for (index, entry) in entries.iter().enumerate() {
        check_occurrence(index, entry, &mut report);
    }
    report.errors
}

/// Collect occurrence diagnostics together with the file that declared them.
///
/// Bundling the declaring path with the error list keeps one extra argument on
/// every check instead of two, and makes it impossible to push a diagnostic
/// that silently omits the source.
struct OccurrenceReport<'a> {
    /// File that declared the occurrence, absent for a source with no path.
    source: Option<&'a Utf8Path>,
    /// Collected diagnostics, in the order the checks found them.
    errors: Vec<Arc<OrthoError>>,
}

impl<'a> OccurrenceReport<'a> {
    /// Start a report for one occurrence in `source`.
    const fn new(source: Option<&'a Utf8Path>) -> Self {
        Self {
            source,
            errors: Vec::new(),
        }
    }

    /// Record `message` against `key`, naming the declaring file when known.
    fn reject(&mut self, key: &str, message: &str) {
        self.errors.push(self.error(key, message));
    }

    /// Build one diagnostic without recording it.
    fn error(&self, key: &str, message: &str) -> Arc<OrthoError> {
        validation_error(key, &self.decorate(message))
    }

    /// Append the declaring-file clause to `message`.
    fn decorate(&self, message: &str) -> String {
        self.source.map_or_else(
            || message.to_owned(),
            |path| format!("{message}; declared in {}", preview(path.as_str())),
        )
    }
}

/// Validate one `shells[index]` entry as a table of exactly three fields.
fn check_occurrence(index: usize, entry: &Value, report: &mut OccurrenceReport<'_>) {
    let Some(fields) = entry.as_object() else {
        report.reject(
            &format!("{SHELLS_KEY}[{index}]"),
            "must be a table declaring name, executable, and args",
        );
        return;
    };
    reject_unknown_fields(index, fields, report);
    check_text_field::<ShellName>(index, "name", fields, report);
    check_text_field::<ShellExecutable>(index, "executable", fields, report);
    check_arguments(index, fields, report);
}

/// Reject any declared field outside the three a definition may carry.
fn reject_unknown_fields(
    index: usize,
    fields: &Map<String, Value>,
    report: &mut OccurrenceReport<'_>,
) {
    for field in fields.keys() {
        if DEFINITION_FIELDS.contains(&field.as_str()) {
            continue;
        }
        report.reject(
            &format!("{SHELLS_KEY}[{index}].{}", bounded(field)),
            "is not a field of a shell definition; expected name, executable, and args",
        );
    }
}

/// Validate one string-valued field through its own deserializer.
///
/// The shape check runs first so a wrong type produces a fixed message instead
/// of a serde message that quotes the rejected value.
fn check_text_field<T: DeserializeOwned>(
    index: usize,
    field: &str,
    fields: &Map<String, Value>,
    report: &mut OccurrenceReport<'_>,
) {
    let Some(value) = fields.get(field) else {
        report.reject(&field_key(index, field), MISSING_FIELD_MESSAGE);
        return;
    };
    if !value.is_string() {
        report.reject(&field_key(index, field), "must be a string");
        return;
    }
    if let Err(error) = serde_json::from_value::<T>(value.clone()) {
        report.reject(&field_key(index, field), &error.to_string());
    }
}

/// Validate `args` as a list of strings, then as an argument vector.
///
/// A non-string element is reported by position and never echoed, because the
/// element is arbitrary text that a diagnostic must not print.
fn check_arguments(index: usize, fields: &Map<String, Value>, report: &mut OccurrenceReport<'_>) {
    let Some(value) = fields.get("args") else {
        report.reject(&field_key(index, "args"), MISSING_FIELD_MESSAGE);
        return;
    };
    let Some(elements) = value.as_array() else {
        report.reject(&field_key(index, "args"), "must be a list of strings");
        return;
    };
    if let Some(position) = elements.iter().position(|element| !element.is_string()) {
        let message = format!("argument {position} must be a string");
        report.reject(&field_key(index, "args"), &message);
        return;
    }
    if let Err(error) = serde_json::from_value::<ShellArguments>(value.clone()) {
        report.reject(&field_key(index, "args"), &error.to_string());
    }
}

/// State that a definition must declare every field and one is absent.
const MISSING_FIELD_MESSAGE: &str =
    "is required; a shell definition must declare name, executable, and args";

/// Build the diagnostic key for one field of one occurrence.
fn field_key(index: usize, field: &str) -> String {
    format!("{SHELLS_KEY}[{index}].{field}")
}
