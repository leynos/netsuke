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
//! Validation and rendering are separate. A check yields an
//! [`OccurrenceDiagnostic`], which carries the field key and the rule the
//! occurrence broke; [`OccurrenceDiagnostic::render`] is the single place that
//! adds the declaring file and bounds the quoted path. Every diagnostic names
//! that file, because the same key and index appear in several layers and the
//! operator needs to know which file to edit, and the path is escaped exactly
//! as a rejected value is, so an unusual directory name cannot break the
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

/// State that a definition must declare every field and one is absent.
const MISSING_FIELD_MESSAGE: &str =
    "is required; a shell definition must declare name, executable, and args";

/// State the trust boundary that reserves `shells` for operator configuration.
const UNTRUSTED_SHELLS_MESSAGE: &str = concat!(
    "the `shells` key may only be declared in an operator configuration file; ",
    "project, explicit, and environment sources are not trusted to declare it"
);

/// Report whether a validation key names the shell registry or a definition.
///
/// Discovery mints exactly two spellings for this schema, the bare `shells`
/// key for an untrusted declaration and `shells[index].field` for a malformed
/// operator one, so the grammar is owned here beside the checks that produce
/// it rather than restated wherever a caller needs to recognise those keys.
/// Diagnostic-mode resolution asks this question to decide whether a failure
/// must defer until `json` is known.
///
/// This stays out of `crate::cli::config::shells` deliberately: the build
/// script compiles that module as part of its own slice for manual-page and
/// completion generation, where no caller exists, and rustc's unused-item
/// analysis in that slice is kept meaningful on purpose.
pub(crate) fn is_shells_key(key: &str) -> bool {
    key == SHELLS_KEY
        || key
            .strip_prefix(SHELLS_KEY)
            .is_some_and(|rest| rest.starts_with('['))
}

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
    OccurrenceDiagnostic::new(SHELLS_KEY, UNTRUSTED_SHELLS_MESSAGE).render(source)
}

/// Validate every occurrence of `shells` in one trusted file layer.
///
/// An absent key yields no errors, because the field defaults to an empty
/// registry. Every fault is collected rather than thrown, so an operator sees
/// all of a file's mistakes in one run.
pub(super) fn occurrence_errors(value: &Value, source: Option<&Utf8Path>) -> Vec<Arc<OrthoError>> {
    occurrence_diagnostics(value)
        .into_iter()
        .map(|diagnostic| diagnostic.render(source))
        .collect()
}

/// Collect every diagnostic one layer's `shells` value warrants.
///
/// This is the query half of the module: it decides what is wrong without
/// deciding how a message should look.
fn occurrence_diagnostics(value: &Value) -> Vec<OccurrenceDiagnostic> {
    let Some(declared) = value.get(SHELLS_KEY) else {
        return Vec::new();
    };
    let Some(entries) = declared.as_array() else {
        return vec![OccurrenceDiagnostic::new(
            SHELLS_KEY,
            "must be a list of shell definitions",
        )];
    };
    let mut diagnostics = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        check_occurrence(index, entry, &mut diagnostics);
    }
    diagnostics
}

/// One rejected field of one occurrence, before the declaring file is named.
///
/// Separating the rule from its rendering lets a test assert the exact key and
/// rule text without a temporary file, and leaves one implementation of the
/// declaring-file clause instead of one per check.
#[derive(Debug, Clone, PartialEq, Eq)]
struct OccurrenceDiagnostic {
    /// Validation key naming the rejected field, such as `shells[0].name`.
    key: String,
    /// Rule the occurrence violated, without the declaring-file clause.
    rule: String,
}

impl OccurrenceDiagnostic {
    /// State one rejected `key` and the `rule` it violated.
    fn new(key: impl Into<String>, rule: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            rule: rule.into(),
        }
    }

    /// Render the diagnostic as a validation error naming `source` when known.
    fn render(self, source: Option<&Utf8Path>) -> Arc<OrthoError> {
        let Self { key, rule } = self;
        let message = match source {
            Some(path) => format!("{rule}; declared in {}", preview(path.as_str())),
            None => rule,
        };
        validation_error(&key, &message)
    }
}

/// Validate one `shells[index]` entry as a table of exactly three fields.
fn check_occurrence(index: usize, entry: &Value, diagnostics: &mut Vec<OccurrenceDiagnostic>) {
    let Some(fields) = entry.as_object() else {
        diagnostics.push(OccurrenceDiagnostic::new(
            format!("{SHELLS_KEY}[{index}]"),
            "must be a table declaring name, executable, and args",
        ));
        return;
    };
    reject_unknown_fields(index, fields, diagnostics);
    check_text_field::<ShellName>(index, "name", fields, diagnostics);
    check_text_field::<ShellExecutable>(index, "executable", fields, diagnostics);
    check_arguments(index, fields, diagnostics);
}

/// Reject any declared field outside the three a definition may carry.
fn reject_unknown_fields(
    index: usize,
    fields: &Map<String, Value>,
    diagnostics: &mut Vec<OccurrenceDiagnostic>,
) {
    for field in fields.keys() {
        if DEFINITION_FIELDS.contains(&field.as_str()) {
            continue;
        }
        diagnostics.push(OccurrenceDiagnostic::new(
            format!("{SHELLS_KEY}[{index}].{}", bounded(field)),
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
    diagnostics: &mut Vec<OccurrenceDiagnostic>,
) {
    let Some(value) = fields.get(field) else {
        diagnostics.push(OccurrenceDiagnostic::new(
            field_key(index, field),
            MISSING_FIELD_MESSAGE,
        ));
        return;
    };
    if !value.is_string() {
        diagnostics.push(OccurrenceDiagnostic::new(
            field_key(index, field),
            "must be a string",
        ));
        return;
    }
    if let Err(error) = serde_json::from_value::<T>(value.clone()) {
        diagnostics.push(OccurrenceDiagnostic::new(
            field_key(index, field),
            error.to_string(),
        ));
    }
}

/// Validate `args` as a list of strings, then as an argument vector.
///
/// A non-string element is reported by position and never echoed, because the
/// element is arbitrary text that a diagnostic must not print.
fn check_arguments(
    index: usize,
    fields: &Map<String, Value>,
    diagnostics: &mut Vec<OccurrenceDiagnostic>,
) {
    let Some(value) = fields.get("args") else {
        diagnostics.push(OccurrenceDiagnostic::new(
            field_key(index, "args"),
            MISSING_FIELD_MESSAGE,
        ));
        return;
    };
    let Some(elements) = value.as_array() else {
        diagnostics.push(OccurrenceDiagnostic::new(
            field_key(index, "args"),
            "must be a list of strings",
        ));
        return;
    };
    if let Some(position) = elements.iter().position(|element| !element.is_string()) {
        diagnostics.push(OccurrenceDiagnostic::new(
            field_key(index, "args"),
            format!("argument {position} must be a string"),
        ));
        return;
    }
    if let Err(error) = serde_json::from_value::<ShellArguments>(value.clone()) {
        diagnostics.push(OccurrenceDiagnostic::new(
            field_key(index, "args"),
            error.to_string(),
        ));
    }
}

/// Build the diagnostic key for one field of one occurrence.
fn field_key(index: usize, field: &str) -> String {
    format!("{SHELLS_KEY}[{index}].{field}")
}

#[cfg(test)]
mod tests {
    //! Unit tests for occurrence validation, with rendering applied.
    //!
    //! Each case names the exact field key and rule text, so removing or
    //! weakening a shape check fails a test rather than silently widening what
    //! an operator may declare.

    use super::*;
    use serde_json::json;

    /// State that a definition is well formed apart from the field under test.
    fn well_formed() -> serde_json::Map<String, Value> {
        json!({
            "name": "dash",
            "executable": "dash",
            "args": ["-c"],
        })
        .as_object()
        .cloned()
        .expect("the fixture literal is an object")
    }

    /// Wrap one entry list as a configuration document declaring `shells`.
    fn declaring(entries: &Value) -> Value {
        json!({ SHELLS_KEY: entries })
    }

    /// Return the key and message of the single diagnostic `value` warrants.
    fn single_diagnostic(value: &Value) -> (String, String) {
        let errors = occurrence_errors(value, None);
        assert_eq!(errors.len(), 1, "expected one diagnostic: {errors:?}");
        let error = errors.first().expect("asserted exactly one diagnostic");
        validation_parts(error)
    }

    /// Split one validation error into its key and message.
    fn validation_parts(error: &OrthoError) -> (String, String) {
        let OrthoError::Validation { key, message } = error else {
            panic!("expected a validation error, got {error:?}");
        };
        (key.clone(), message.clone())
    }

    #[test]
    fn unknown_field_is_rejected_by_name() {
        let mut entry = well_formed();
        entry.insert("executabl".to_owned(), json!("dash"));

        let (key, message) = single_diagnostic(&declaring(&json!([entry])));

        assert_eq!(key, "shells[0].executabl");
        assert!(
            message.contains("is not a field of a shell definition"),
            "unexpected message: {message}"
        );
    }

    #[test]
    fn missing_name_is_reported_against_its_key() {
        let mut entry = well_formed();
        entry.remove("name");

        let (key, message) = single_diagnostic(&declaring(&json!([entry])));

        assert_eq!(key, "shells[0].name");
        assert!(message.starts_with("is required"), "unexpected: {message}");
    }

    #[test]
    fn missing_executable_is_reported_against_its_key() {
        let mut entry = well_formed();
        entry.remove("executable");

        let (key, message) = single_diagnostic(&declaring(&json!([entry])));

        assert_eq!(key, "shells[0].executable");
        assert!(message.starts_with("is required"), "unexpected: {message}");
    }

    #[test]
    fn missing_args_is_reported_against_its_key() {
        let mut entry = well_formed();
        entry.remove("args");

        let (key, message) = single_diagnostic(&declaring(&json!([entry])));

        assert_eq!(key, "shells[0].args");
        assert!(message.starts_with("is required"), "unexpected: {message}");
    }

    #[test]
    fn non_list_shells_is_rejected_at_the_registry_key() {
        let (key, message) = single_diagnostic(&declaring(&json!("dash")));

        assert_eq!(key, "shells");
        assert_eq!(message, "must be a list of shell definitions");
    }

    #[test]
    fn non_table_entry_is_rejected_at_its_index() {
        let (key, message) = single_diagnostic(&declaring(&json!(["dash"])));

        assert_eq!(key, "shells[0]");
        assert_eq!(
            message,
            "must be a table declaring name, executable, and args"
        );
    }

    #[test]
    fn wrong_typed_name_is_rejected_without_echoing_it() {
        let mut entry = well_formed();
        entry.insert("name".to_owned(), json!(1));

        let (key, message) = single_diagnostic(&declaring(&json!([entry])));

        assert_eq!(key, "shells[0].name");
        assert_eq!(message, "must be a string");
    }

    #[test]
    fn wrong_typed_arguments_are_rejected_without_echoing_them() {
        let mut entry = well_formed();
        entry.insert("args".to_owned(), json!("-c"));

        let (key, message) = single_diagnostic(&declaring(&json!([entry])));

        assert_eq!(key, "shells[0].args");
        assert_eq!(message, "must be a list of strings");
    }

    #[test]
    fn every_fault_in_one_entry_is_reported() {
        let value = declaring(&json!([{
            "name": 1,
            "args": ["-c"],
            "extra": true,
        }]));

        let keys = occurrence_errors(&value, None)
            .iter()
            .map(|error| validation_parts(error).0)
            .collect::<Vec<_>>();

        assert_eq!(
            keys,
            vec!["shells[0].extra", "shells[0].name", "shells[0].executable"]
        );
    }

    #[test]
    fn an_absent_key_yields_no_diagnostic() {
        assert!(occurrence_errors(&json!({"jobs": 2}), None).is_empty());
        assert!(!declares_shells(&json!({"jobs": 2})));
        assert!(declares_shells(&declaring(&json!([]))));
    }

    #[test]
    fn rendering_names_the_declaring_file_once() {
        let source = Utf8Path::new("/etc/netsuke/config.toml");
        let errors = occurrence_errors(&declaring(&json!("dash")), Some(source));
        let error = errors.first().expect("expected one diagnostic");
        let (key, message) = validation_parts(error);

        assert_eq!(key, "shells");
        assert_eq!(
            message,
            "must be a list of shell definitions; declared in \"/etc/netsuke/config.toml\""
        );
    }
}
