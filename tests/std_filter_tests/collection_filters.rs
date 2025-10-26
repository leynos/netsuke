//! Behavioural coverage for the `MiniJinja` collection filters exposed by the
//! Netsuke stdlib.
//!
//! These tests exercise the filters end-to-end through a configured template
//! environment to ensure we keep parity between unit expectations and rendered
//! output, especially across error handling scenarios.
use anyhow::{bail, ensure, Context, Result};
use minijinja::{context, value::Value, ErrorKind};
use rstest::rstest;
use serde::Serialize;

use super::support::fallible;

#[rstest]
fn uniq_removes_duplicate_strings() -> Result<()> {
    let mut env = fallible::stdlib_env()?;
    fallible::register_template(&mut env, "uniq", "{{ values | uniq | join(',') }}")?;
    let template = env
        .get_template("uniq")
        .context("fetch template 'uniq'")?;
    let output = template
        .render(context!(values => vec!["a", "a", "b", "b", "c"]))
        .context("render template 'uniq'")?;
    ensure!(
        output == "a,b,c",
        "uniq should collapse duplicates, but rendered {output}"
    );
    Ok(())
}

#[rstest]
fn uniq_rejects_non_iterables() -> Result<()> {
    let env = fallible::stdlib_env()?;
    let err = match env.render_str("{{ value | uniq }}", context!(value => 1)) {
        Ok(output) => bail!("expected uniq to reject scalars but rendered {output}"),
        Err(err) => err,
    };
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "uniq should report InvalidOperation, but was {:?}",
        err.kind()
    );
    ensure!(
        err.to_string().contains("is not iterable"),
        "error should mention non-iterable input: {err}"
    );
    Ok(())
}

#[rstest]
fn flatten_flattens_deeply_nested_lists() -> Result<()> {
    let mut env = fallible::stdlib_env()?;
    fallible::register_template(&mut env, "flatten", "{{ values | flatten | join(',') }}")?;
    let template = env
        .get_template("flatten")
        .context("fetch template 'flatten'")?;
    let output = template
        .render(context!(values => vec![vec![vec!["one"], vec!["two"]], vec![vec!["three"]]]))
        .context("render template 'flatten'")?;
    ensure!(
        output == "one,two,three",
        "flatten should concatenate items, but rendered {output}"
    );
    Ok(())
}

#[rstest]
fn flatten_errors_on_scalar_items() -> Result<()> {
    let env = fallible::stdlib_env()?;
    let err = match env.render_str("{{ [[1], 2] | flatten }}", context! {}) {
        Ok(output) => bail!(
            "expected flatten to reject scalar items but rendered {output}"
        ),
        Err(err) => err,
    };
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "flatten should report InvalidOperation, but was {:?}",
        err.kind()
    );
    ensure!(
        err.to_string().contains("flatten expected sequence items"),
        "error should describe the invalid item: {err}"
    );
    Ok(())
}

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
    ensure!(output == "2", "expected two 'tool' items but rendered {output}");
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
    ensure!(output == "1,2", "expected '1,2' ordering but rendered {output}");
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
    ensure!(output == "2", "expected two entries with key 1 but rendered {output}");
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
        Ok(output) => bail!(
            "expected group_by to fail ({description}), but rendered {output}"
        ),
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
