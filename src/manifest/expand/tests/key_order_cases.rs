//! Key-order regressions for `foreach` and `when` removal.

use super::*;
use anyhow::{Result, ensure};
use googletest::prelude::*;
use pretty_assertions::{assert_eq, assert_ne};
use serde_json::json;

/// The non-control fields every case writes, in authoring order.
fn fields() -> Vec<(&'static str, ManifestValue)> {
    vec![
        ("name", json!("ordered")),
        ("vars", json!({"alpha": 1, "beta": 2})),
        ("command", json!("echo ordered")),
        ("description", json!("ordered description")),
    ]
}

/// Build an entry with `control` inserted at `position` among four fields.
fn entry_with_control(position: usize, control: (&str, ManifestValue)) -> ManifestMap {
    let (control_key, control_value) = control;
    let mut map = ManifestMap::new();
    let fields = fields();
    let field_count = fields.len();
    for (index, (key, value)) in fields.into_iter().enumerate() {
        if index == position {
            map.insert(control_key.into(), control_value.clone());
        }
        map.insert(key.into(), value);
    }
    if position == field_count {
        map.insert(control_key.into(), control_value);
    }
    map
}

/// Expand one entry in `section`, returning the entries it produced.
fn expand_one(section: &str, entry: ManifestMap) -> Result<Vec<ManifestValue>> {
    let env = Environment::new();
    let mut document = ManifestMap::new();
    document.insert(
        section.into(),
        ManifestValue::Array(vec![ManifestValue::Object(entry)]),
    );
    let mut doc = ManifestValue::Object(document);
    expand_foreach(&mut doc, &env)?;
    Ok(section_entries(&doc, section)?.to_vec())
}

/// Assert one expanded entry keeps the authoring key order and `vars` shape.
fn assert_entry_order(entry: &ManifestValue, expected_vars: &[&str]) -> Result<()> {
    let map = entry.as_object().context("expanded entry map")?;
    let keys: Vec<&str> = map.keys().map(String::as_str).collect();
    assert_that!(
        keys,
        elements_are![&"name", &"vars", &"command", &"description"]
    );
    let vars = map
        .get("vars")
        .and_then(ManifestValue::as_object)
        .context("expanded vars map")?;
    let vars_keys: Vec<&str> = vars.keys().map(String::as_str).collect();
    assert_eq!(vars_keys, expected_vars);
    Ok(())
}

#[rstest::rstest]
#[case::targets_first("targets", 0)]
#[case::targets_second("targets", 1)]
#[case::targets_third("targets", 2)]
#[case::targets_fourth("targets", 3)]
#[case::targets_last("targets", 4)]
#[case::actions_first("actions", 0)]
#[case::actions_second("actions", 1)]
#[case::actions_third("actions", 2)]
#[case::actions_fourth("actions", 3)]
#[case::actions_last("actions", 4)]
fn static_when_removal_keeps_authoring_key_order(
    #[case] section: &str,
    #[case] position: usize,
) -> Result<()> {
    let entry = entry_with_control(position, ("when", json!("true")));

    let entries = expand_one(section, entry)?;

    ensure!(
        entries.len() == 1,
        "a true condition must keep exactly one {section} entry, got {}",
        entries.len()
    );
    let kept = entries
        .first()
        .context("a true condition must keep its entry")?;
    assert_entry_order(kept, &["alpha", "beta"])
}

#[rstest::rstest]
#[case::targets_first("targets", 0)]
#[case::targets_second("targets", 1)]
#[case::targets_third("targets", 2)]
#[case::targets_fourth("targets", 3)]
#[case::targets_last("targets", 4)]
#[case::actions_first("actions", 0)]
#[case::actions_second("actions", 1)]
#[case::actions_third("actions", 2)]
#[case::actions_fourth("actions", 3)]
#[case::actions_last("actions", 4)]
fn foreach_removal_keeps_authoring_key_order(
    #[case] section: &str,
    #[case] position: usize,
) -> Result<()> {
    let entry = entry_with_control(position, ("foreach", json!([1, 2])));

    let entries = expand_one(section, entry)?;

    ensure!(
        entries.len() == 2,
        "two iteration values must produce two {section} entries, got {}",
        entries.len()
    );
    for candidate in &entries {
        assert_entry_order(candidate, &["alpha", "beta", "item", "index"])?;
    }
    Ok(())
}

#[test]
fn map_equality_ignores_key_order() {
    let mut left = ManifestMap::new();
    left.insert("first".into(), json!(1));
    left.insert("second".into(), json!(2));
    let mut right = ManifestMap::new();
    right.insert("second".into(), json!(2));
    right.insert("first".into(), json!(1));

    assert_ne!(
        left.keys().collect::<Vec<_>>(),
        right.keys().collect::<Vec<_>>(),
        "the two maps must hold the same keys in different orders"
    );
    assert_eq!(
        left, right,
        "map equality delegates to `IndexMap`, which ignores key order"
    );
}
