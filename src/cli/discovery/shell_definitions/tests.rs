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
