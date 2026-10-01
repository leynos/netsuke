//! Behavioural coverage for the `compact` filter.
//!
//! `compact` drops `none`, undefined and the empty string, and keeps every
//! other value — including `0`, `false`, whitespace and the empty containers,
//! which a truthiness reading of "blank" would discard. The cases here pin that
//! boundary by example; the `OBL-COMPACT` property in `compact_property` pins
//! it over the generated domain.
//!
//! The subject's *kind* matters as much as its members. `compact` accepts a
//! [`ValueKind::Seq`] and a [`ValueKind::Iterable`] and refuses everything
//! else, so [`compact_accepts_a_real_iterable_subject`] reaches the second arm
//! through [`Value::make_iterable`] rather than through a rendered list, and
//! [`compact_rejects_an_undefined_subject`] passes [`Value::UNDEFINED`] as the
//! subject itself rather than as a member.

use anyhow::{Context, Result, bail, ensure};
use minijinja::{
    ErrorKind, context,
    value::{Value, ValueKind},
};
use rstest::rstest;
use test_support::fluent::normalize_fluent_isolates;

use super::fallible;

/// `OBL-COMPACT`'s witness: `0` and `false` survive, the blank members do not.
///
/// All three droppable kinds appear here as members — `none`, undefined, and
/// the empty string — because they are distinct kinds reached by distinct
/// predicates (`is_none`, `is_undefined`, and a `ValueKind::String` guard on
/// emptiness), and a `join` would not distinguish a member that was dropped
/// from one that rendered as nothing.
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
        Value::UNDEFINED,
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

/// An empty byte array is a value, not an empty string.
///
/// `Value::as_str` answers for well-formed UTF-8 bytes as well as strings, so
/// a predicate written as `value.as_str().is_some_and(str::is_empty)` discards
/// `Value::from_bytes(vec![])` under a rule that only ever meant empty text.
/// The two kinds are distinct here, and the length check distinguishes them
/// without depending on how either renders.
#[test]
fn compact_retains_an_empty_byte_array() -> Result<()> {
    let env = fallible::stdlib_env()?;
    let values = vec![Value::from_bytes(Vec::new()), Value::from("")];
    let output = env
        .render_str(
            "{{ values | compact | length }}",
            context!(values => values),
        )
        .context("render compact over an empty byte array")?;
    ensure!(
        output == "1",
        "compact must retain the empty byte array and drop only the empty string, but the \
         surviving length was {output}"
    );
    Ok(())
}

/// An explicit `none` member is droppable, not merely a coerced absence.
///
/// `Value::from(())` is a genuine `none`, a different kind from
/// [`Value::UNDEFINED`] — which the witness case above carries, so the two
/// predicates are pinned separately rather than by one member standing in for
/// both.
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
///
/// The `undefined` case is the one a template reaches by naming a variable that
/// was never bound. `Value::from(())` is a genuine `none` and is a different
/// kind, so the two get their own rows: an implementation that folded undefined
/// into none would name the wrong kind here.
#[rstest]
#[case::number("1", "number")]
#[case::boolean("true", "bool")]
#[case::string("'abc'", "string")]
#[case::none("none", "none")]
#[case::undefined("unbound_name", "undefined")]
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

/// `compact` accepts a real iterable subject, not only a sequence.
///
/// `compact_filter` admits `ValueKind::Seq` and `ValueKind::Iterable` and
/// refuses everything else, but every other case here reaches it through a
/// rendered list — which is a `Seq`. A MiniJinja `range()` reports
/// `ObjectRepr::Iterable` instead, so it is the only subject that exercises the
/// second arm of that match; delete the arm and this test stops compiling the
/// same expectation.
///
/// The kind is asserted before the filter runs, and the members afterwards by
/// value and by kind, because a rendered `join` could agree with a filter that
/// had coerced the subject or dropped a member's type on the way through.
#[test]
fn compact_accepts_a_real_iterable_subject() -> Result<()> {
    let env = fallible::stdlib_env()?;
    let subject = Value::make_iterable(|| 0_u32..5_u32);
    ensure!(
        subject.kind() == ValueKind::Iterable,
        "the fixture must be a real iterable, but its kind was {:?}",
        subject.kind()
    );

    let template = env
        .compile_expression("subject | compact | list")
        .context("compile the compact iterable probe")?;
    let rendered = template
        .eval(context!(subject => subject.clone()))
        .context("evaluate the compact iterable probe")?;

    let kept: Vec<Value> = rendered
        .try_iter()
        .context("the compacted iterable should itself be iterable")?
        .collect();
    let expected: Vec<Value> = (0_u32..5_u32).map(Value::from).collect();
    ensure!(
        kept == expected,
        "compact must retain every member of an iterable subject in order, so {subject:?} \
         should yield {expected:?}, but yielded {kept:?}"
    );
    ensure!(
        kept.iter().all(|item| item.kind() == ValueKind::Number),
        "every retained member must keep its kind, but got {:?}",
        kept.iter().map(Value::kind).collect::<Vec<_>>()
    );
    Ok(())
}

/// The empty containers are values, and `compact` retains them.
///
/// The predicate's doc comment names `[]` and `{}` among the things it keeps,
/// and no other case in this suite reaches either: `compact_accepts_an_empty_\
/// sequence` passes an empty sequence as the *subject*, which proves the gate
/// admits it, not that the filter keeps it as a *member*. A truthiness reading
/// of "blank" drops both, so this is that defect's negative control.
///
/// Order is asserted, and so are the kinds. The rendered text alone would not
/// do: `[]` renders as `[]` and `{}` as `{}`, but an implementation that
/// replaced either with a string would render something else only by accident,
/// whereas the kind check fails it outright.
#[test]
fn compact_retains_the_empty_containers() -> Result<()> {
    let env = fallible::stdlib_env()?;
    let empty_list = Value::from(Vec::<Value>::new());
    let empty_map = Value::from_iter(Vec::<(String, Value)>::new());
    ensure!(
        empty_list.kind() == ValueKind::Seq,
        "the first fixture must be an empty sequence, but was {:?}",
        empty_list.kind()
    );
    ensure!(
        empty_map.kind() == ValueKind::Map,
        "the second fixture must be an empty map, but was {:?}",
        empty_map.kind()
    );

    let subject = vec![
        empty_list.clone(),
        Value::from(""),
        empty_map.clone(),
        Value::from(""),
    ];
    let template = env
        .compile_expression("subject | compact | list")
        .context("compile the empty-container probe")?;
    let rendered = template
        .eval(context!(subject => subject.clone()))
        .context("evaluate the empty-container probe")?;

    let kept: Vec<Value> = rendered
        .try_iter()
        .context("the compacted containers should themselves be iterable")?
        .collect();
    ensure!(
        kept == vec![empty_list.clone(), empty_map.clone()],
        "compact must retain the empty sequence and the empty map, in order, and drop only \
         the two empty strings, but {subject:?} yielded {kept:?}"
    );
    ensure!(
        kept.iter().map(Value::kind).collect::<Vec<_>>() == vec![ValueKind::Seq, ValueKind::Map],
        "the retained containers must keep their kinds, seq then map, but got {:?}",
        kept.iter().map(Value::kind).collect::<Vec<_>>()
    );
    ensure!(
        kept[0]
            .try_iter()
            .is_ok_and(|mut members| members.next().is_none()),
        "the retained sequence must still be empty, but was {:?}",
        kept[0]
    );
    ensure!(
        kept[1]
            .try_iter()
            .is_ok_and(|mut members| members.next().is_none()),
        "the retained map must still be empty, but was {:?}",
        kept[1]
    );
    Ok(())
}
