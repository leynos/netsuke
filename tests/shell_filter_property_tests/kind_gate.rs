//! OBL-KIND-GATE: the sequence and string filters refuse the wrong kinds.
//!
//! Both filters could be written on `Value::try_iter`, which accepts a map
//! (yielding keys) and a string (yielding characters). The obligation is that
//! they refuse those instead, and name the received kind in a diagnostic a
//! manifest author can act on. [`try_iter_would_have_accepted_three_of_the_rejected_subjects`]
//! is the control: it shows the underlying iterator *would* have produced
//! members, so the refusal is the kind gate's doing and not the value being
//! uniterable.
use super::property_support::{SH, render_with};
use anyhow::{Context, Result, bail, ensure};
use minijinja::value::Value;
use rstest::rstest;

// ---------------------------------------------------------------------------
// OBL-KIND-GATE
// ---------------------------------------------------------------------------

/// One rejected subject: the value to bind and the kind it should be named as.
///
/// The kind names are the spellings `ValueKind`'s `Display` produces, because
/// the diagnostics interpolate that `Display` and the assertion is on the
/// message a manifest author actually reads.
struct Rejected {
    /// Builds the subject bound to the template's `subject` variable.
    build: fn() -> Value,
    /// The kind name the diagnostic must carry.
    kind: &'static str,
    /// What the subject is, for the failure message.
    description: &'static str,
}

/// Build a `MiniJinja` value that is iterable but not a `Seq`.
///
/// `range(3)` is the template-visible form of this: it is an object reporting
/// `ObjectRepr::Iterable`, so `ValueKind` calls it `iterator` and the gate
/// rejects it, while `try_iter` would hand back `0, 1, 2`.
fn iterable_object() -> Value {
    Value::make_iterable(|| 0_u32..3_u32)
}

/// Subjects `compact` and `shell_join` must reject, with their kind names.
///
/// `map` and `string` are the two D8 exists for: `Value::try_iter()` accepts
/// both — a map yields its keys, a string yields its characters — so an
/// implementation that skipped the kind gate would quietly transform them
/// rather than refusing.
const NON_SEQUENCES: &[Rejected] = &[
    Rejected {
        build: || Value::from_iter(std::iter::once(("a", Value::from(1)))),
        kind: "map",
        description: "a mapping",
    },
    Rejected {
        build: || Value::from("abc"),
        kind: "string",
        description: "a string",
    },
    Rejected {
        build: || Value::from(()),
        kind: "none",
        description: "none",
    },
    Rejected {
        build: || Value::UNDEFINED,
        kind: "undefined",
        description: "an undefined name",
    },
    Rejected {
        build: || Value::from(7),
        kind: "number",
        description: "a number",
    },
    Rejected {
        build: || Value::from(true),
        kind: "bool",
        description: "a boolean",
    },
];

/// Subjects `shell_quote` must reject, with their kind names.
const NON_STRINGS: &[Rejected] = &[
    Rejected {
        build: || Value::from_iter(std::iter::once(("a", Value::from(1)))),
        kind: "map",
        description: "a mapping",
    },
    Rejected {
        build: || Value::from_iter([Value::from(1), Value::from(2)]),
        kind: "sequence",
        description: "a sequence",
    },
    Rejected {
        build: || Value::from(()),
        kind: "none",
        description: "none",
    },
    Rejected {
        build: || Value::UNDEFINED,
        kind: "undefined",
        description: "an undefined name",
    },
    Rejected {
        build: || Value::from(7),
        kind: "number",
        description: "a number",
    },
    Rejected {
        build: || Value::from(true),
        kind: "bool",
        description: "a boolean",
    },
    Rejected {
        build: iterable_object,
        kind: "iterator",
        description: "a MiniJinja object with no string form",
    },
];

/// Render `{{ subject | filter }}` and return the error text.
///
/// A successful render is a failure of the test: the whole point is that the
/// filter refuses, so a value that came back is reported as the surprise it is.
fn rejection(filter: &str, subject: &Value) -> Result<String> {
    let template = format!("{{{{ value | {filter} }}}}");
    match render_with(&template, SH, subject) {
        Ok(rendered) => bail!("{{ value | {filter} }} should have been rejected, got {rendered:?}"),
        Err(error) => Ok(format!("{error:#}")),
    }
}

/// Both sequence filters refuse a non-sequence and name what they got.
///
/// The two filters word the diagnostic differently — `compact expects a
/// sequence` against `shell_join expects a sequence` — but both carry the
/// received kind, which is what the case asserts alongside the expected text.
#[rstest]
#[case::compact("compact", "compact expects a sequence")]
#[case::shell_join("shell_join", "shell_join expects a sequence")]
fn sequence_filters_reject_non_sequences(
    #[case] filter: &str,
    #[case] expectation: &str,
    #[values(
        "a mapping",
        "a string",
        "none",
        "an undefined name",
        "a number",
        "a boolean"
    )]
    description: &str,
) -> Result<()> {
    let rejected = NON_SEQUENCES
        .iter()
        .find(|candidate| candidate.description == description)
        .with_context(|| format!("no rejection fixture for {description}"))?;
    let reported = rejection(filter, &(rejected.build)())?;
    ensure!(
        reported.contains(expectation),
        "{filter} should report {expectation:?} for {description}: {reported}"
    );
    ensure!(
        reported.contains(rejected.kind),
        "{filter} should name the kind {:?} for {description}: {reported}",
        rejected.kind
    );
    Ok(())
}

/// `shell_quote` refuses every non-string subject, including a Jinja object.
///
/// The `args` code is asserted here and not on `compact` because only the
/// recipe-text filters carry it: `compact`'s diagnostic is a collections
/// message with no `netsuke::jinja::shell::args` prefix, and asserting one
/// would be asserting a code that does not exist.
#[rstest]
#[case("a mapping")]
#[case("a sequence")]
#[case("none")]
#[case("an undefined name")]
#[case("a number")]
#[case("a boolean")]
#[case("a MiniJinja object with no string form")]
fn shell_quote_rejects_non_strings(#[case] description: &str) -> Result<()> {
    let rejected = NON_STRINGS
        .iter()
        .find(|candidate| candidate.description == description)
        .with_context(|| format!("no rejection fixture for {description}"))?;
    let reported = rejection("shell_quote", &(rejected.build)())?;
    ensure!(
        reported.contains("shell_quote expects a string"),
        "shell_quote should state its expectation for {description}: {reported}"
    );
    ensure!(
        reported.contains(rejected.kind),
        "shell_quote should name the kind {:?} for {description}: {reported}",
        rejected.kind
    );
    ensure!(
        reported.contains("netsuke::jinja::shell::args"),
        "shell_quote should carry the args code for {description}: {reported}"
    );
    assert_no_stringification(&reported, description);
    Ok(())
}

/// Assert the diagnostic quotes the kind rather than stringifying the value.
///
/// A `to_string` fallback would render a number as `7` inside a *successful*
/// quote, and a boolean as `true`; the diagnostic naming the kind instead is
/// how the caller learns the filter refused rather than coerced. Kept as a
/// helper so the single call site reads as one assertion.
fn assert_no_stringification(reported: &str, description: &str) {
    assert!(
        !reported.contains("expected string"),
        "shell_quote must not fall back to stringifying {description}: {reported}"
    );
}

/// The kind gate is doing work `try_iter` would not do.
///
/// This is the negative control for the two cases above. `try_iter` accepts a
/// map (yielding keys), a string (yielding characters), and an iterable object,
/// so an implementation gated on it alone would accept all three. Showing that
/// the underlying iterator *would* have produced something is what makes the
/// rejection the gate's doing rather than the value being uniterable.
#[test]
fn try_iter_would_have_accepted_three_of_the_rejected_subjects() -> Result<()> {
    for (subject, name) in [
        (
            Value::from_iter(std::iter::once(("a", Value::from(1)))),
            "a mapping",
        ),
        (Value::from("abc"), "a string"),
        (iterable_object(), "an iterable object"),
    ] {
        let items = subject
            .try_iter()
            .with_context(|| format!("{name} should be iterable, or this control proves nothing"))?
            .count();
        ensure!(
            items > 0,
            "{name} should yield members, or this control proves nothing"
        );
    }
    // And the gate refuses a mapping anyway, which is the contrast: the
    // rejection is the kind check's doing, not the value being uniterable.
    ensure!(
        rejection(
            "compact",
            &Value::from_iter(std::iter::once(("a", Value::from(1))))
        )?
        .contains("map"),
        "compact must reject a mapping rather than iterate its keys"
    );
    Ok(())
}
