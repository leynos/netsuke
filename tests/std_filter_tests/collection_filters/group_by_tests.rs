//! Behavioural coverage for the `group_by` filter.
//!
//! The filter clusters sequence items by the resolved value of an attribute,
//! preserving first-key-seen order, and must reject both a blank attribute and
//! an item that carries no value for it.

use anyhow::{Context, Result, bail, ensure};
use minijinja::{ErrorKind, context, value::Value};
use rstest::rstest;
use serde::Serialize;

use super::fallible;

#[derive(Debug, Serialize)]
struct Item<'a> {
    class: &'a str,
    name: &'a str,
}

#[rstest]
fn group_by_partitions_struct_fields() -> Result<()> {
    let env = fallible::stdlib_env()?;
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
        .context("render group_by by struct attribute")?;
    ensure!(output == "2:1", "expected '2:1' but rendered {output}");
    Ok(())
}

#[rstest]
fn group_by_reads_mapping_entries() -> Result<()> {
    let env = fallible::stdlib_env()?;
    let template = "{{ (values | group_by('kind')).tool | length }}";
    let values = vec![
        context!(kind => "tool", name => "saw"),
        context!(kind => "tool", name => "plane"),
        context!(kind => "material", name => "oak"),
    ];
    let output = env
        .render_str(template, context!(values => values))
        .context("render group_by on mapping items")?;
    ensure!(
        output == "2",
        "expected two 'tool' items but rendered {output}"
    );
    Ok(())
}

#[rstest]
fn group_by_preserves_insertion_order() -> Result<()> {
    let env = fallible::stdlib_env()?;
    let template = "{{ values | group_by('kind') | list | join(',') }}";
    let values = vec![context!(kind => 1), context!(kind => 2)];
    let output = env
        .render_str(template, context!(values => values))
        .context("render group_by preserves order")?;
    ensure!(
        output == "1,2",
        "expected '1,2' ordering but rendered {output}"
    );
    Ok(())
}

#[rstest]
fn group_by_supports_non_string_keys() -> Result<()> {
    #[derive(Serialize)]
    struct Item {
        kind: Value,
    }

    let env = fallible::stdlib_env()?;
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
        .context("render group_by with non-string keys")?;
    ensure!(
        output == "2",
        "expected two entries with key 1 but rendered {output}"
    );
    Ok(())
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
) -> Result<()> {
    let env = fallible::stdlib_env()?;
    let template = format!("{{{{ values | group_by('{attribute}') }}}}");
    let result = env.render_str(
        &template,
        context!(values => vec![Item { class: "a", name: "alpha" }]),
    );
    let err = match result {
        Ok(output) => bail!("expected group_by to fail ({description}), but rendered {output}"),
        Err(err) => err,
    };
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "group_by should report InvalidOperation, but was {:?}",
        err.kind()
    );
    ensure!(
        err.to_string().contains(expected_fragment),
        "error should mention `{expected_fragment}` but was: {err}"
    );
    Ok(())
}
