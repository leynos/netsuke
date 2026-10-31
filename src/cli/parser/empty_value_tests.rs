//! Tests for the empty-value boundary in the CLI parse path.
//!
//! `ortho_config` drops empty strings from clap's context before building
//! Fluent arguments, so [`is_empty_value_validation`] recognises that shape at
//! the parser boundary and keeps the validator's own diagnostic. These cases
//! pin the predicate apart from the localization rendering tests.

use super::*;

/// Build a `ValueValidation` error carrying `value` as its only context.
fn value_validation_with(value: ContextValue) -> clap::Error {
    let mut error = clap::Error::raw(ErrorKind::ValueValidation, "rejected");
    error.insert(ContextKind::InvalidValue, value);
    error
}

/// An empty single supplied value must be recognised at the parse boundary.
#[test]
fn empty_single_value_is_recognised() {
    let error = value_validation_with(ContextValue::String(String::new()));
    assert!(
        is_empty_value_validation(&error),
        "an empty supplied value should be recognised"
    );
}

/// A non-empty supplied value still belongs to `ortho_config`'s template path.
#[test]
fn non_empty_single_value_is_not_recognised() {
    let error = value_validation_with(ContextValue::String("bogus".to_owned()));
    assert!(
        !is_empty_value_validation(&error),
        "a non-empty supplied value should keep the catalogue template path"
    );
}

/// A multi-value list whose entries are all empty loses the template argument.
#[test]
fn all_empty_entries_in_multi_value_list_are_recognised() {
    let error = value_validation_with(ContextValue::Strings(vec![String::new(), String::new()]));
    assert!(
        is_empty_value_validation(&error),
        "a multi-value list of empty entries should be recognised"
    );
}

/// A mixed list still joins to a non-empty value, so the template can render.
#[test]
fn mixed_entries_in_multi_value_list_are_not_recognised() {
    let error = value_validation_with(ContextValue::Strings(vec![
        "first".to_owned(),
        String::new(),
    ]));
    assert!(
        !is_empty_value_validation(&error),
        "a mixed multi-value list should keep the catalogue template path"
    );
}

/// A `ValueValidation` error with no supplied value is not this defect.
#[test]
fn missing_value_context_is_not_recognised() {
    let error = clap::Error::raw(ErrorKind::ValueValidation, "rejected");
    assert!(
        !is_empty_value_validation(&error),
        "absent context is not an empty supplied value"
    );
}

/// Other error kinds keep their established localization path.
#[test]
fn other_error_kind_is_not_recognised() {
    let mut error = clap::Error::raw(ErrorKind::InvalidValue, "rejected");
    error.insert(
        ContextKind::InvalidValue,
        ContextValue::String(String::new()),
    );
    assert!(
        !is_empty_value_validation(&error),
        "only value-validation errors take the empty-value boundary path"
    );
}
