//! Tests for the `StringOrList` manifest field type.
//!
//! Covers both how the scalar-or-sequence YAML shape deserializes and how the
//! conversion methods on the type behave across every variant.

use anyhow::{Context, Result, bail, ensure};
use netsuke::ast::StringOrList;
use rstest::rstest;

use super::support::parse_manifest;

#[test]
fn string_or_list_variants() -> Result<()> {
    {
        let yaml = r#"
            netsuke_version: "1.0.0"
            targets:
              - name: hello
                command: "echo hi"
        "#;
        let manifest = parse_manifest(yaml)?;
        let first = manifest
            .targets
            .first()
            .context("manifest should contain at least one target")?;
        match &first.name {
            StringOrList::String(name) => {
                ensure!(name == "hello", "unexpected name: {name}");
            }
            other => bail!("Expected String variant, got: {other:?}"),
        }
    }

    {
        let yaml = r#"
            netsuke_version: "1.0.0"
            targets:
              - name:
                  - hello
                  - world
                command: "echo hi"
        "#;
        let manifest = parse_manifest(yaml)?;
        let first = manifest
            .targets
            .first()
            .context("manifest should contain at least one target")?;
        match &first.name {
            StringOrList::List(names) => {
                let expected = vec!["hello".to_owned(), "world".to_owned()];
                ensure!(
                    names == &expected,
                    "unexpected names: got {:?}, expected {:?}",
                    names,
                    expected
                );
            }
            other => bail!("Expected List variant, got: {other:?}"),
        }
    }

    {
        let yaml = r#"
            netsuke_version: "1.0.0"
            targets:
              - name: []
                command: "echo hi"
        "#;
        let manifest = parse_manifest(yaml)?;
        let first = manifest
            .targets
            .first()
            .context("manifest should contain at least one target")?;
        match &first.name {
            StringOrList::List(names) => {
                ensure!(names.is_empty(), "expected empty list, got {names:?}");
            }
            other => bail!("Expected List variant, got: {other:?}"),
        }
    }
    Ok(())
}

#[rstest]
#[case(StringOrList::Empty, &[])]
#[case(StringOrList::String("cc".into()), &["cc"])]
#[case(StringOrList::List(vec!["cc".into(), "ld".into()]), &["cc", "ld"])]
fn string_or_list_to_string_vec(#[case] value: StringOrList, #[case] expected: &[&str]) {
    assert_eq!(
        value.to_string_vec(),
        expected,
        "to_string_vec mismatch for {value:?}"
    );
}

#[rstest]
#[case(StringOrList::Empty, None)]
#[case(StringOrList::String("cc".into()), Some("cc"))]
#[case(StringOrList::List(vec!["cc".into()]), Some("cc"))]
#[case(StringOrList::List(Vec::new()), None)]
#[case(StringOrList::List(vec!["cc".into(), "ld".into()]), None)]
fn string_or_list_as_single(#[case] value: StringOrList, #[case] expected: Option<&str>) {
    assert_eq!(
        value.as_single(),
        expected,
        "as_single mismatch for {value:?}"
    );
}

#[rstest]
#[case(StringOrList::Empty, &[])]
#[case(StringOrList::String("hello".into()), &[5])]
#[case(StringOrList::List(Vec::new()), &[])]
#[case(StringOrList::List(vec!["a".into(), "abc".into()]), &[1, 3])]
fn string_or_list_map_each(#[case] value: StringOrList, #[case] expected: &[usize]) {
    assert_eq!(
        value.map_each(str::len),
        expected,
        "map_each mismatch for {value:?}"
    );
}
