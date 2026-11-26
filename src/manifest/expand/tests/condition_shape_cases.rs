//! Witness cases for every condition shape expansion evaluates.
//!
//! The generated properties state the same shapes over random values; these
//! fixed cases pin one example per shape, operator, and form. A regression in
//! a shape's spelling is then visible without reading a shrunk
//! counter-example, and the integer-truthiness shape gets an explicit zero
//! case rather than relying on a draw.

use super::*;
use minijinja::Environment;
use rstest::rstest;
use serde_json::json;

/// Expand one entry and report whether its condition kept it.
fn survives(
    when: &str,
    vars: Option<ManifestValue>,
    foreach: Option<ManifestValue>,
) -> Result<bool> {
    let environment = Environment::new();
    let mut entry = ManifestMap::new();
    entry.insert("name".into(), json!("witness"));
    if let Some(vars) = vars {
        entry.insert("vars".into(), vars);
    }
    if let Some(foreach) = foreach {
        entry.insert("foreach".into(), foreach);
    }
    entry.insert("when".into(), json!(when));
    let mut document = json!({"targets": [ManifestValue::Object(entry)]});
    expand_foreach(&mut document, &environment)?;
    Ok(!targets(&document)?.is_empty())
}

/// Expand one foreach entry and return the indexes that survived.
fn kept_indexes(when: &str, foreach: ManifestValue) -> Result<Vec<u64>> {
    let environment = Environment::new();
    let mut entry = ManifestMap::new();
    entry.insert("name".into(), json!("witness"));
    entry.insert("foreach".into(), foreach);
    entry.insert("when".into(), json!(when));
    let mut document = json!({"targets": [ManifestValue::Object(entry)]});
    expand_foreach(&mut document, &environment)?;
    indexes(targets(&document)?, "targets")
}

/// Build a `vars` object holding one integer.
fn integer_var(name: &str, value: i64) -> ManifestValue {
    ManifestValue::Object(ManifestMap::from_iter([(
        name.to_owned(),
        ManifestValue::Number(value.into()),
    )]))
}

#[rstest]
#[case::literal_true("true", true)]
#[case::literal_false("false", false)]
#[case::literal_true_template("{{ true }}", true)]
#[case::literal_false_template("{{ false }}", false)]
fn literal_conditions(#[case] when: &str, #[case] expected: bool) -> Result<()> {
    anyhow::ensure!(
        survives(when, None, None)? == expected,
        "`{when}` should {} the entry",
        if expected { "keep" } else { "drop" }
    );
    Ok(())
}

#[rstest]
#[case::flag_true("w0", true, true)]
#[case::flag_false("w0", false, false)]
#[case::flag_true_template("{{ w0 }}", true, true)]
#[case::flag_false_template("{{ w0 }}", false, false)]
fn boolean_variable_conditions(
    #[case] when: &str,
    #[case] value: bool,
    #[case] expected: bool,
) -> Result<()> {
    let vars = ManifestValue::Object(ManifestMap::from_iter([(
        "w0".to_owned(),
        ManifestValue::Bool(value),
    )]));
    anyhow::ensure!(survives(when, Some(vars), None)? == expected);
    Ok(())
}

#[rstest]
#[case::negated_true("not w0", true, false)]
#[case::negated_false("not w0", false, true)]
#[case::negated_true_template("{{ not w0 }}", true, false)]
#[case::negated_false_template("{{ not w0 }}", false, true)]
fn negated_variable_conditions(
    #[case] when: &str,
    #[case] value: bool,
    #[case] expected: bool,
) -> Result<()> {
    let vars = ManifestValue::Object(ManifestMap::from_iter([(
        "w0".to_owned(),
        ManifestValue::Bool(value),
    )]));
    anyhow::ensure!(survives(when, Some(vars), None)? == expected);
    Ok(())
}

#[rstest]
#[case::zero_is_false(0, false)]
#[case::one_is_true(1, true)]
#[case::negative_is_true(-1, true)]
fn integer_truthiness_conditions(#[case] value: i64, #[case] expected: bool) -> Result<()> {
    anyhow::ensure!(
        survives("w0", Some(integer_var("w0", value)), None)? == expected,
        "integer {value} used as a condition should {} the entry",
        if expected { "keep" } else { "drop" }
    );
    Ok(())
}

#[rstest]
#[case::lt(" < ", 3, true)]
#[case::lt_false(" < ", 1, false)]
#[case::le(" <= ", 2, true)]
#[case::le_false(" <= ", 1, false)]
#[case::eq(" == ", 2, true)]
#[case::eq_false(" == ", 3, false)]
#[case::ne(" != ", 3, true)]
#[case::ne_false(" != ", 2, false)]
#[case::gt(" > ", 1, true)]
#[case::gt_false(" > ", 2, false)]
#[case::ge(" >= ", 2, true)]
#[case::ge_false(" >= ", 3, false)]
fn variable_comparisons(
    #[case] operator: &str,
    #[case] constant: i64,
    #[case] expected: bool,
) -> Result<()> {
    let when = format!("w0{operator}{constant}");
    anyhow::ensure!(survives(&when, Some(integer_var("w0", 2)), None)? == expected);
    let template = format!("{{{{ w0{operator}{constant} }}}}");
    anyhow::ensure!(survives(&template, Some(integer_var("w0", 2)), None)? == expected);
    Ok(())
}

#[rstest]
#[case::lt(" < ", 2, &[0, 1])]
#[case::le(" <= ", 1, &[0, 1])]
#[case::eq(" == ", 2, &[2])]
#[case::ne(" != ", 2, &[0, 1, 3])]
#[case::gt(" > ", 1, &[2, 3])]
#[case::ge(" >= ", 2, &[2, 3])]
fn index_comparisons(
    #[case] operator: &str,
    #[case] constant: i64,
    #[case] expected: &[u64],
) -> Result<()> {
    let items = json!([0, 1, 2, 3]);
    let expression = format!("index{operator}{constant}");
    anyhow::ensure!(kept_indexes(&expression, items.clone())? == expected);
    let template = format!("{{{{ index{operator}{constant} }}}}");
    anyhow::ensure!(kept_indexes(&template, items)? == expected);
    Ok(())
}

#[rstest]
#[case::lt(" < ", 4, &[0, 3])]
#[case::le(" <= ", 1, &[0])]
#[case::eq(" == ", 4, &[1, 2])]
#[case::ne(" != ", 4, &[0, 3])]
#[case::gt(" > ", 1, &[1, 2, 3])]
#[case::ge(" >= ", 4, &[1, 2])]
fn item_comparisons(
    #[case] operator: &str,
    #[case] constant: i64,
    #[case] expected: &[u64],
) -> Result<()> {
    let items = json!([1, 4, 4, 2]);
    let expression = format!("item{operator}{constant}");
    anyhow::ensure!(kept_indexes(&expression, items.clone())? == expected);
    let template = format!("{{{{ item{operator}{constant} }}}}");
    anyhow::ensure!(kept_indexes(&template, items)? == expected);
    Ok(())
}
