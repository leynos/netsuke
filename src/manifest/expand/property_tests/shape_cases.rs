//! Witness cases for every condition shape expansion evaluates.
//!
//! The generated properties state the same shapes over random values; these
//! fixed cases pin one example per shape, operator, and form. A regression in
//! a shape's spelling is then visible without reading a shrunk
//! counter-example, and the integer-truthiness shape gets an explicit zero
//! case rather than relying on a draw.
//!
//! The cases share one `ConditionCase` table rather than repeating the same
//! four-line body per group: the interesting content is the table, and a copy
//! per group would drift when the expected rendering rules change. Each row
//! also states which spellings are meaningful, because an integer-valued
//! condition has no template form: the fallback reads a rendered integer as
//! false, so testing one in template form would assert the wrong contract.

use super::super::expand_foreach;
use super::super::tests::{indexes, targets};
use crate::manifest::{ManifestMap, ManifestValue};
use anyhow::Result;
use minijinja::Environment;
use rstest::rstest;
use serde_json::json;

/// One fixed condition, the bindings it reads, and its expected verdict.
struct ConditionCase {
    /// The condition source, written in expression form.
    expression: &'static str,
    /// Which spellings the condition has a meaningful form in.
    forms: Forms,
    /// The `vars` value the entry carries, if any.
    vars: Option<ManifestValue>,
    /// The `foreach` value the entry carries, if any.
    foreach: Option<ManifestValue>,
    /// Whether the condition should keep the entry, or the indexes it keeps.
    verdict: Verdict,
}

/// Which spellings of a condition are meaningful.
///
/// A template renders to a string, and the fallback accepts only `true` or
/// `1`. An integer-valued condition therefore has no template form: `{{ w0 }}`
/// rendering `-1` would read as false, so testing it in that form would assert
/// the wrong contract.
#[derive(Clone, Copy)]
enum Forms {
    /// Expression and template spellings both mean the same thing.
    Both,
    /// Only the expression spelling is meaningful.
    ExpressionOnly,
}

/// The expected outcome of one witness case.
enum Verdict {
    /// A static entry: the condition keeps or drops it.
    Keeps(bool),
    /// A foreach entry: the surviving indexes.
    Indexes(&'static [u64]),
}

impl ConditionCase {
    /// Run the case in every spelling that is meaningful for its shape.
    fn check(&self) -> Result<()> {
        self.check_form(self.expression)?;
        if matches!(self.forms, Forms::Both) {
            self.check_form(&format!("{{{{ {} }}}}", self.expression))?;
        }
        Ok(())
    }

    /// Run the case with one spelling of the condition.
    fn check_form(&self, when: &str) -> Result<()> {
        match self.verdict {
            Verdict::Keeps(expected) => {
                anyhow::ensure!(
                    survives(when, self.vars.clone(), self.foreach.clone())? == expected,
                    "`{when}` should {} the entry",
                    if expected { "keep" } else { "drop" }
                );
            }
            Verdict::Indexes(expected) => {
                let items = self
                    .foreach
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("`{when}` needs a foreach value"))?;
                anyhow::ensure!(
                    kept_indexes(when, items)? == expected,
                    "`{when}` should keep {expected:?}"
                );
            }
        }
        Ok(())
    }
}

/// Build a `vars` object holding one boolean `w0`.
fn with_bool(value: bool) -> ManifestValue {
    vars_with_bool(value)
}

/// Build a `vars` object holding one integer `w0`.
fn with_integer(value: i64) -> ManifestValue {
    ManifestValue::Object(ManifestMap::from_iter([(
        "w0".to_owned(),
        ManifestValue::Number(value.into()),
    )]))
}

/// Build a `vars` object holding one boolean `w0`.
fn vars_with_bool(value: bool) -> ManifestValue {
    ManifestValue::Object(ManifestMap::from_iter([(
        "w0".to_owned(),
        ManifestValue::Bool(value),
    )]))
}

/// Run one table row.
fn run(case: &ConditionCase) -> Result<()> {
    case.check()
}

#[rstest]
#[case(ConditionCase { expression: "true", forms: Forms::Both, vars: None, foreach: None, verdict: Verdict::Keeps(true) })]
#[case(ConditionCase { expression: "false", forms: Forms::Both, vars: None, foreach: None, verdict: Verdict::Keeps(false) })]
fn literal_conditions(#[case] case: ConditionCase) -> Result<()> {
    run(&case)
}

#[rstest]
#[case(ConditionCase { expression: "w0", forms: Forms::Both, vars: Some(with_bool(true)), foreach: None, verdict: Verdict::Keeps(true) })]
#[case(ConditionCase { expression: "w0", forms: Forms::Both, vars: Some(with_bool(false)), foreach: None, verdict: Verdict::Keeps(false) })]
fn boolean_variable_conditions(#[case] case: ConditionCase) -> Result<()> {
    run(&case)
}

#[rstest]
#[case(ConditionCase { expression: "not w0", forms: Forms::Both, vars: Some(with_bool(true)), foreach: None, verdict: Verdict::Keeps(false) })]
#[case(ConditionCase { expression: "not w0", forms: Forms::Both, vars: Some(with_bool(false)), foreach: None, verdict: Verdict::Keeps(true) })]
fn negated_variable_conditions(#[case] case: ConditionCase) -> Result<()> {
    run(&case)
}

#[rstest]
#[case(ConditionCase { expression: "w0", forms: Forms::ExpressionOnly, vars: Some(with_integer(0)), foreach: None, verdict: Verdict::Keeps(false) })]
#[case(ConditionCase { expression: "w0", forms: Forms::ExpressionOnly, vars: Some(with_integer(1)), foreach: None, verdict: Verdict::Keeps(true) })]
#[case(ConditionCase { expression: "w0", forms: Forms::ExpressionOnly, vars: Some(with_integer(-1)), foreach: None, verdict: Verdict::Keeps(true) })]
fn integer_truthiness_conditions(#[case] case: ConditionCase) -> Result<()> {
    run(&case)
}

#[rstest]
#[case(ConditionCase { expression: "w0 < 3", forms: Forms::Both, vars: Some(with_integer(2)), foreach: None, verdict: Verdict::Keeps(true) })]
#[case(ConditionCase { expression: "w0 < 1", forms: Forms::Both, vars: Some(with_integer(2)), foreach: None, verdict: Verdict::Keeps(false) })]
#[case(ConditionCase { expression: "w0 <= 2", forms: Forms::Both, vars: Some(with_integer(2)), foreach: None, verdict: Verdict::Keeps(true) })]
#[case(ConditionCase { expression: "w0 <= 1", forms: Forms::Both, vars: Some(with_integer(2)), foreach: None, verdict: Verdict::Keeps(false) })]
#[case(ConditionCase { expression: "w0 == 2", forms: Forms::Both, vars: Some(with_integer(2)), foreach: None, verdict: Verdict::Keeps(true) })]
#[case(ConditionCase { expression: "w0 == 3", forms: Forms::Both, vars: Some(with_integer(2)), foreach: None, verdict: Verdict::Keeps(false) })]
#[case(ConditionCase { expression: "w0 != 3", forms: Forms::Both, vars: Some(with_integer(2)), foreach: None, verdict: Verdict::Keeps(true) })]
#[case(ConditionCase { expression: "w0 != 2", forms: Forms::Both, vars: Some(with_integer(2)), foreach: None, verdict: Verdict::Keeps(false) })]
#[case(ConditionCase { expression: "w0 > 1", forms: Forms::Both, vars: Some(with_integer(2)), foreach: None, verdict: Verdict::Keeps(true) })]
#[case(ConditionCase { expression: "w0 > 2", forms: Forms::Both, vars: Some(with_integer(2)), foreach: None, verdict: Verdict::Keeps(false) })]
#[case(ConditionCase { expression: "w0 >= 2", forms: Forms::Both, vars: Some(with_integer(2)), foreach: None, verdict: Verdict::Keeps(true) })]
#[case(ConditionCase { expression: "w0 >= 3", forms: Forms::Both, vars: Some(with_integer(2)), foreach: None, verdict: Verdict::Keeps(false) })]
fn variable_comparisons(#[case] case: ConditionCase) -> Result<()> {
    run(&case)
}

/// Build a foreach case over `items` keeping the listed indexes.
fn over_indexes(expression: &'static str, indexes: &'static [u64]) -> ConditionCase {
    ConditionCase {
        expression,
        forms: Forms::Both,
        vars: None,
        foreach: Some(json!([0, 1, 2, 3])),
        verdict: Verdict::Indexes(indexes),
    }
}

#[rstest]
#[case(over_indexes("index < 2", &[0, 1]))]
#[case(over_indexes("index <= 1", &[0, 1]))]
#[case(over_indexes("index == 2", &[2]))]
#[case(over_indexes("index != 2", &[0, 1, 3]))]
#[case(over_indexes("index > 1", &[2, 3]))]
#[case(over_indexes("index >= 2", &[2, 3]))]
fn index_comparisons(#[case] case: ConditionCase) -> Result<()> {
    run(&case)
}

/// Build a foreach case over `1, 4, 4, 2` keeping the listed indexes.
fn over_items(expression: &'static str, indexes: &'static [u64]) -> ConditionCase {
    ConditionCase {
        expression,
        forms: Forms::Both,
        vars: None,
        foreach: Some(json!([1, 4, 4, 2])),
        verdict: Verdict::Indexes(indexes),
    }
}

#[rstest]
#[case(over_items("item < 4", &[0, 3]))]
#[case(over_items("item <= 1", &[0]))]
#[case(over_items("item == 4", &[1, 2]))]
#[case(over_items("item != 4", &[0, 3]))]
#[case(over_items("item > 1", &[1, 2, 3]))]
#[case(over_items("item >= 4", &[1, 2]))]
fn item_comparisons(#[case] case: ConditionCase) -> Result<()> {
    run(&case)
}

/// Expand one entry and report whether its condition kept it.
fn survives(
    when: &str,
    vars_value: Option<ManifestValue>,
    foreach_value: Option<ManifestValue>,
) -> Result<bool> {
    let mut document = witness_document(when, vars_value, foreach_value, false);
    let environment = Environment::new();
    expand_foreach(&mut document, &environment)?;
    Ok(!targets(&document)?.is_empty())
}

/// Expand one foreach entry and return the indexes that survived.
fn kept_indexes(when: &str, foreach: ManifestValue) -> Result<Vec<u64>> {
    let mut document = witness_document(when, None, Some(foreach), true);
    let environment = Environment::new();
    expand_foreach(&mut document, &environment)?;
    indexes(targets(&document)?, "targets")
}

/// Build a one-entry document holding the condition under test.
fn witness_document(
    when: &str,
    vars_value: Option<ManifestValue>,
    foreach_value: Option<ManifestValue>,
    iterates: bool,
) -> ManifestValue {
    let mut entry = ManifestMap::new();
    entry.insert("name".into(), json!("witness"));
    if let Some(vars) = vars_value {
        entry.insert("vars".into(), vars);
    }
    if iterates && let Some(foreach) = foreach_value {
        entry.insert("foreach".into(), foreach);
    }
    entry.insert("when".into(), json!(when));
    json!({"targets": [ManifestValue::Object(entry)]})
}
