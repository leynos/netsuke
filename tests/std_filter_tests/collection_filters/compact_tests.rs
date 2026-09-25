//! Behavioural coverage for the `compact` filter.
//!
//! `compact` drops `none`, undefined and the empty string, and keeps every
//! other value — including `0`, `false` and whitespace, which a truthiness
//! reading of "blank" would discard. The cases here pin that boundary by
//! example; the `OBL-COMPACT` property in `compact_property` pins it over the
//! generated domain.

use anyhow::{Context, Result, bail, ensure};
use minijinja::{ErrorKind, context, value::Value};
use rstest::rstest;
use test_support::fluent::normalize_fluent_isolates;

use super::fallible;

/// `OBL-COMPACT`'s witness: `0` and `false` survive, the blank members do not.
///
/// The expectation is written out rather than computed from the filter's own
/// predicate, so this cannot agree with a redefinition of "blank". It is also
/// the naive-truthiness negative control: an implementation that dropped
/// members by `!item.is_true()` renders `x` here. The `False` spelling is
/// `join`'s, which renders a boolean the Python way; what this pins is *which*
/// members survive, and that is exactly the three that do.
#[test]
fn compact_drops_witness_case_blanks_only() -> Result<()> {
    let env = fallible::stdlib_env()?;
    let values = vec![
        Value::from(0),
        Value::from(false),
        Value::from(""),
        Value::from(()),
        Value::from("x"),
    ];
    let output = env
        .render_str(
            "{{ values | compact | join(',') }}",
            context!(values => values),
        )
        .context("render the compact witness case")?;
    ensure!(
        output == "0,False,x",
        "compact must drop only none, undefined and the empty string, but rendered {output}"
    );
    Ok(())
}

/// An explicit `none` member is droppable, not merely a coerced absence.
#[test]
fn compact_drops_an_injected_none_member() -> Result<()> {
    let env = fallible::stdlib_env()?;
    let values = vec![Value::from("keep"), Value::from(()), Value::from("also")];
    let output = env
        .render_str(
            "{{ values | compact | join(',') }}",
            context!(values => values),
        )
        .context("render compact over an injected none")?;
    ensure!(
        output == "keep,also",
        "an injected none must be dropped, but rendered {output}"
    );
    Ok(())
}

/// `compact` rejects a subject that is not a sequence, naming what it received.
///
/// Per D8 the check is on `ValueKind`, not on `try_iter()`: a map and a string
/// both iterate happily — a map over its keys, a string over its characters — so
/// an implementation that merely tried to iterate would quietly accept them.
#[rstest]
#[case::number("1", "number")]
#[case::boolean("true", "bool")]
#[case::string("'abc'", "string")]
#[case::none("none", "none")]
#[case::mapping("{'a': 1}", "map")]
fn compact_rejects_non_sequences(#[case] subject: &str, #[case] expected_kind: &str) -> Result<()> {
    let env = fallible::stdlib_env()?;
    let template = format!("{{{{ {subject} | compact }}}}");
    let Err(err) = env.render_str(&template, context! {}) else {
        bail!("compact must reject a {expected_kind} subject");
    };
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "compact should report InvalidOperation, but was {:?}",
        err.kind()
    );
    let message = normalize_fluent_isolates(&err.to_string());
    ensure!(
        message.contains("compact expects a sequence"),
        "error should name the expectation: {message}"
    );
    ensure!(
        message.contains(expected_kind),
        "error should name the received kind `{expected_kind}`: {message}"
    );
    Ok(())
}

/// A sequence of length zero is a sequence, not a non-sequence.
#[test]
fn compact_accepts_an_empty_sequence() -> Result<()> {
    let env = fallible::stdlib_env()?;
    let output = env
        .render_str("{{ [] | compact | length }}", context! {})
        .context("render compact over the empty sequence")?;
    ensure!(
        output == "0",
        "an empty sequence must render 0, got {output}"
    );
    Ok(())
}
