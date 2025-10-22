#![allow(clippy::expect_used, reason = "collection filter tests prefer expect for clarity")]

//! Behavioural coverage for the `MiniJinja` collection filters exposed by the
//! Netsuke stdlib.
//!
//! These tests exercise the filters end-to-end through a configured template
//! environment to ensure we keep parity between unit expectations and rendered
//! output, especially across error handling scenarios.
use minijinja::{ErrorKind, context, value::Value};
use rstest::rstest;
use serde::Serialize;

use super::support::{register_template, stdlib_env};

#[rstest]
fn uniq_removes_duplicate_strings() {
    let mut env = stdlib_env();
    register_template(&mut env, "uniq", "{{ values | uniq | join(',') }}");
    let template = env.get_template("uniq").expect("template");
    let output = template
        .render(context!(values => vec!["a", "a", "b", "b", "c"]))
        .expect("render uniq");
    assert_eq!(output, "a,b,c");
}

#[rstest]
fn uniq_rejects_non_iterables() {
    let env = stdlib_env();
    let result = env.render_str("{{ value | uniq }}", context!(value => 1));
    let err = result.expect_err("uniq should reject scalars");
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    assert!(
        err.to_string().contains("is not iterable"),
        "error should mention non-iterable input: {err}",
    );
}

#[rstest]
fn flatten_flattens_deeply_nested_lists() {
    let mut env = stdlib_env();
    register_template(&mut env, "flatten", "{{ values | flatten | join(',') }}");
    let template = env.get_template("flatten").expect("template");
    let output = template
        .render(context!(values => vec![vec![vec!["one"], vec!["two"]], vec![vec!["three"]]]))
        .expect("render flatten");
    assert_eq!(output, "one,two,three");
}

#[rstest]
fn flatten_errors_on_scalar_items() {
    let env = stdlib_env();
    let result = env.render_str("{{ [[1], 2] | flatten }}", context! {});
    let err = result.expect_err("flatten should reject scalar items");
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    assert!(
        err.to_string().contains("flatten expected sequence items"),
        "error should describe the invalid item: {err}",
    );
}

#[derive(Debug, Serialize)]
struct Item<'a> {
    class: &'a str,
    name: &'a str,
}

#[rstest]
fn group_by_partitions_struct_fields() {
    let env = stdlib_env();
    let template = "{{ (values | group_by('class')).a | length }}:{{ (values | group_by('class')).b | length }}";
    let values = vec![
        Item {
            class: "a",
            name: "first",
        },
        Item {
            class: "a",
            name: "second",
        },
        Item {
            class: "b",
            name: "third",
        },
    ];
    let output = env
        .render_str(template, context!(values => values))
        .expect("render group_by");
    assert_eq!(output, "2:1");
}

#[rstest]
fn group_by_reads_mapping_entries() {
    let env = stdlib_env();
    let template = "{{ (values | group_by('kind')).tool | length }}";
    let values = vec![
        context!(kind => "tool", name => "saw"),
        context!(kind => "tool", name => "plane"),
        context!(kind => "material", name => "oak"),
    ];
    let output = env
        .render_str(template, context!(values => values))
        .expect("render group_by mappings");
    assert_eq!(output, "2");
}

#[rstest]
fn group_by_preserves_insertion_order() {
    let env = stdlib_env();
    let template = "{{ values | group_by('kind') | list | join(',') }}";
    let values = vec![context!(kind => 1), context!(kind => 2)];
    let output = env
        .render_str(template, context!(values => values))
        .expect("render group_by ordering");
    assert_eq!(output, "1,2");
}

#[rstest]
fn group_by_supports_non_string_keys() {
    #[derive(Serialize)]
    struct Item {
        kind: Value,
    }

    let env = stdlib_env();
    let template = "{{ (values | group_by('kind'))[1] | length }}";
    let values = vec![
        Item {
            kind: Value::from(1),
        },
        Item {
            kind: Value::from(1),
        },
        Item {
            kind: Value::from(2),
        },
    ];
    let output = env
        .render_str(template, context!(values => values))
        .expect("render group_by non-string keys");
    assert_eq!(output, "2");
}

#[rstest]
#[case("", "non-empty attribute", "group_by should reject empty attribute")]
#[case(
    "missing",
    "could not resolve",
    "group_by should reject missing attribute"
)]
fn group_by_errors_for_invalid_attributes(
    #[case] attribute: &str,
    #[case] expected_fragment: &str,
    #[case] description: &str,
) {
    let env = stdlib_env();
    let template = format!("{{{{ values | group_by('{attribute}') }}}}");
    let result = env.render_str(
        &template,
        context!(values => vec![Item { class: "a", name: "alpha" }]),
    );
    let err = result.expect_err(description);
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    assert!(
        err.to_string().contains(expected_fragment),
        "error should contain fragment `{expected_fragment}` but was: {err}",
    );
}
