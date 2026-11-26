//! Unhappy-path cases for a `when` condition that expansion must reject.
//!
//! `when` accepts only a non-empty string. A YAML boolean, integer, null, list,
//! or mapping is rejected rather than coerced, in both sections and for static
//! and `foreach` entries alike. Undefined names split two ways, which the cases
//! below measure: a bare name is false, while a name that an operation or a
//! template consumes is an error under the strict environment.

use super::*;
use anyhow::ensure;
use minijinja::{Environment, UndefinedBehavior};
use rstest::rstest;
use serde_json::{Value as JsonValue, json};

/// The localized message expansion produces for a non-string `when`.
const NOT_STRING: &str = "must be a string.";

/// Build the production-shaped environment: strict undefined, no helpers.
fn strict_environment() -> Environment<'static> {
    let mut environment = Environment::new();
    environment.set_undefined_behavior(UndefinedBehavior::Strict);
    environment
}

/// Expand one entry whose `when` holds `when_value`, reporting the error.
fn expand_with_when(
    section: &str,
    when_value: JsonValue,
    is_foreach: bool,
) -> std::result::Result<ExpansionReport, String> {
    let mut entry = ManifestMap::new();
    entry.insert("name".into(), json!("subject"));
    entry.insert("command".into(), json!("echo subject"));
    if is_foreach {
        entry.insert("foreach".into(), json!([1]));
    }
    entry.insert("when".into(), when_value);
    let mut object = ManifestMap::new();
    object.insert(
        section.into(),
        JsonValue::Array(vec![JsonValue::Object(entry)]),
    );
    let mut document = JsonValue::Object(object);
    expand_foreach(&mut document, &strict_environment()).map_err(|error| format!("{error:#}"))
}

/// Build the section and entry-kind matrix every case iterates over.
fn when_type_matrix() -> [(&'static str, bool); 4] {
    [
        ("targets", false),
        ("targets", true),
        ("actions", false),
        ("actions", true),
    ]
}

/// Verify a non-string `when` is rejected in both sections and both kinds.
#[test]
fn non_string_when_is_rejected_in_every_section_and_kind() -> Result<()> {
    for (section, is_foreach) in when_type_matrix() {
        for when_value in [
            json!(true),
            json!(0),
            JsonValue::Null,
            json!(["yes"]),
            json!({"nested": true}),
        ] {
            let outcome = expand_with_when(section, when_value.clone(), is_foreach).map(|_| ());
            let message = outcome.expect_err("a non-string `when` must be rejected");
            ensure!(
                message.contains(NOT_STRING),
                "expected `{NOT_STRING}` for {when_value} in {section} \
                 (foreach: {is_foreach}), got: {message}"
            );
        }
    }
    Ok(())
}

/// Verify a bare undefined name is falsy rather than an error.
///
/// `MiniJinja` evaluates a bare name to an undefined value, and its truthiness
/// is false. The strict-undefined setting bites when the value is consumed, as
/// the next test shows, not when it is merely tested.
#[test]
fn a_bare_undefined_name_excludes_the_entry() -> Result<()> {
    for (section, is_foreach) in when_type_matrix() {
        let outcome = expand_with_when(section, json!("missing_flag"), is_foreach);
        let report = outcome.map_err(|message| anyhow::anyhow!("{message}"))?;
        ensure!(
            report.stats.filtered_targets + report.stats.filtered_actions == 1,
            "a bare undefined name must exclude exactly one entry in {section} \
             (foreach: {is_foreach})"
        );
    }
    Ok(())
}

/// Verify an undefined name that is consumed is an error, not an exclusion.
#[rstest]
#[case::undefined_in_comparison("missing_count > 1")]
#[case::undefined_template("{{ missing_flag }}")]
fn consumed_undefined_names_are_errors(#[case] when: &str) -> Result<()> {
    for (section, is_foreach) in when_type_matrix() {
        let outcome = expand_with_when(section, json!(when), is_foreach).map(|_| ());
        let message = outcome.expect_err("a consumed undefined name must be an error");
        ensure!(
            !message.is_empty(),
            "the error must carry a diagnostic for `{when}` in {section}"
        );
    }
    Ok(())
}
