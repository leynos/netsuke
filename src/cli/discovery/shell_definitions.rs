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

use ortho_config::OrthoError;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::sync::Arc;

use crate::cli::config::shells::{ShellArguments, ShellExecutable, preview::bounded};
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
pub(super) fn untrusted_shells_error() -> Arc<OrthoError> {
    validation_error(SHELLS_KEY, UNTRUSTED_SHELLS_MESSAGE)
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
pub(super) fn occurrence_errors(value: &Value) -> Vec<Arc<OrthoError>> {
    let mut errors = Vec::new();
    let Some(declared) = value.get(SHELLS_KEY) else {
        return errors;
    };
    let Some(entries) = declared.as_array() else {
        errors.push(validation_error(
            SHELLS_KEY,
            "must be a list of shell definitions",
        ));
        return errors;
    };
    for (index, entry) in entries.iter().enumerate() {
        check_occurrence(index, entry, &mut errors);
    }
    errors
}

/// Validate one `shells[index]` entry as a table of exactly three fields.
fn check_occurrence(index: usize, entry: &Value, errors: &mut Vec<Arc<OrthoError>>) {
    let Some(fields) = entry.as_object() else {
        errors.push(validation_error(
            &format!("{SHELLS_KEY}[{index}]"),
            "must be a table declaring name, executable, and args",
        ));
        return;
    };
    reject_unknown_fields(index, fields, errors);
    check_text_field::<ShellName>(index, "name", fields, errors);
    check_text_field::<ShellExecutable>(index, "executable", fields, errors);
    check_arguments(index, fields, errors);
}

/// Reject any declared field outside the three a definition may carry.
fn reject_unknown_fields(
    index: usize,
    fields: &Map<String, Value>,
    errors: &mut Vec<Arc<OrthoError>>,
) {
    for field in fields.keys() {
        if DEFINITION_FIELDS.contains(&field.as_str()) {
            continue;
        }
        errors.push(validation_error(
            &format!("{SHELLS_KEY}[{index}].{}", bounded(field)),
            "is not a field of a shell definition; expected name, executable, and args",
        ));
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
    errors: &mut Vec<Arc<OrthoError>>,
) {
    let Some(value) = fields.get(field) else {
        errors.push(missing_field(index, field));
        return;
    };
    if !value.is_string() {
        errors.push(validation_error(
            &field_key(index, field),
            "must be a string",
        ));
        return;
    }
    if let Err(error) = serde_json::from_value::<T>(value.clone()) {
        errors.push(validation_error(
            &field_key(index, field),
            &error.to_string(),
        ));
    }
}

/// Validate `args` as a list of strings, then as an argument vector.
///
/// A non-string element is reported by position and never echoed, because the
/// element is arbitrary text that a diagnostic must not print.
fn check_arguments(index: usize, fields: &Map<String, Value>, errors: &mut Vec<Arc<OrthoError>>) {
    let Some(value) = fields.get("args") else {
        errors.push(missing_field(index, "args"));
        return;
    };
    let Some(elements) = value.as_array() else {
        errors.push(validation_error(
            &field_key(index, "args"),
            "must be a list of strings",
        ));
        return;
    };
    if let Some(position) = elements.iter().position(|element| !element.is_string()) {
        errors.push(validation_error(
            &field_key(index, "args"),
            &format!("argument {position} must be a string"),
        ));
        return;
    }
    if let Err(error) = serde_json::from_value::<ShellArguments>(value.clone()) {
        errors.push(validation_error(
            &field_key(index, "args"),
            &error.to_string(),
        ));
    }
}

/// Build the diagnostic key for one field of one occurrence.
fn field_key(index: usize, field: &str) -> String {
    format!("{SHELLS_KEY}[{index}].{field}")
}

/// Report a field a definition must declare and does not.
fn missing_field(index: usize, field: &str) -> Arc<OrthoError> {
    validation_error(
        &field_key(index, field),
        "is required; a shell definition must declare name, executable, and args",
    )
}
