//! Tests for the `StringOrList` manifest field type.
//!
//! Covers both how the scalar-or-sequence YAML shape deserializes and how the
//! conversion methods on the type behave across every variant.
//!
//! The `rstest` tables below pin the `Empty` and `String` variants, which have
//! a single shape each, plus a handful of representative lists. The property
//! tests that follow sweep the `List` variant across arbitrary lengths, where
//! the handwritten cases cannot reach: `map_each` walks lists with an indexed
//! loop, so a fault in that traversal — dropping, duplicating, or reordering
//! an element — only surfaces once list length varies freely.

use anyhow::{Context, Result, bail, ensure};
use netsuke::ast::StringOrList;
use proptest::prelude::*;
use rstest::rstest;

use super::support::parse_manifest;

/// Generate lists of manifest-like tokens, including the empty list.
///
/// Strings are drawn from the character set that manifest names, rules and
/// paths use and kept short; the properties under test are length- and
/// order-sensitive, not content-sensitive, so longer values would only slow
/// the run down.
fn token_lists() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec("[a-zA-Z0-9_./-]{0,8}", 0..16)
}

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

/// Wrap a token so the mapped output is distinguishable from the input.
///
/// Using a mapping other than `str::to_owned` keeps the `map_each` property
/// independent of `to_string_vec`, which is itself defined as
/// `map_each(str::to_owned)`; comparing against the same transformation
/// applied element-wise still pins length and order.
fn decorate(value: &str) -> String {
    format!("<{value}>")
}

proptest! {
    /// `map_each` visits every element exactly once, in order.
    ///
    /// The element-wise expectation is built independently of `map_each`, so a
    /// traversal that skipped, repeated or reordered an entry would diverge.
    #[test]
    fn map_each_preserves_length_and_order(values in token_lists()) {
        let expected: Vec<String> = values.iter().map(|value| decorate(value)).collect();
        let mapped = StringOrList::List(values.clone()).map_each(decorate);
        prop_assert_eq!(
            mapped,
            expected,
            "map_each did not preserve length and order for {:?}",
            values
        );
    }

    /// `to_string_vec` reproduces the list verbatim for any length.
    #[test]
    fn to_string_vec_reproduces_the_list(values in token_lists()) {
        prop_assert_eq!(
            StringOrList::List(values.clone()).to_string_vec(),
            values.clone(),
            "to_string_vec did not reproduce {:?}",
            values
        );
    }

    /// `as_single` yields the sole element for one-element lists and `None`
    /// otherwise, whether the list is empty or holds two or more entries.
    #[test]
    fn as_single_holds_only_for_one_element_lists(values in token_lists()) {
        let expected = if values.len() == 1 {
            values.first().map(String::as_str)
        } else {
            None
        };
        let value = StringOrList::List(values.clone());
        prop_assert_eq!(
            value.as_single(),
            expected,
            "as_single mismatch for a list of {} element(s): {:?}",
            values.len(),
            values
        );
    }
}
