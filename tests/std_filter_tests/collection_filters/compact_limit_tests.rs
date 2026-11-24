//! Consumption limits for the `compact` filter.
//!
//! `compact` used to collect every member of its subject. `MiniJinja`'s lazy
//! sequence repetition (`[none] * n`) makes a short expression name an
//! arbitrarily large logical input, so collecting first let a manifest request
//! unbounded CPU and memory before template evaluation resumed. These tests pin
//! the fix: a standalone registration enforces the documented per-call member
//! ceiling even when no manifest-wide budget exists, and it charges a member
//! before deciding whether to retain it.
//!
//! The counted source below distinguishes the two plausible incorrect
//! implementations. Charging only retained members lets an all-blank
//! repetition run forever; charging after the append lets the retained vector
//! grow past the ceiling before the failure. A hard guard turns either defect
//! into a fast failure rather than a hung test.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use anyhow::{Context, Result, bail, ensure};
use minijinja::{
    ErrorKind, context,
    value::{Value, ValueKind},
};
use rstest::rstest;
use test_support::fluent::normalize_fluent_isolates;

use super::fallible;

/// The documented per-call member ceiling for one collection filter call.
///
/// The integration-test crate sees the public stdlib surface only, so the
/// number is restated here. `docs/users-guide.md` states the same ceiling, and
/// the manifest unit tests pin the crate-internal constant against it.
const PER_CALL_LIMIT: usize = 100_000;

/// A source several times the ceiling, so a bounded filter must stop early.
const OVERSIZED_SOURCE: usize = PER_CALL_LIMIT * 3;

/// Pull members from a shared source, counting and guarding every pull.
///
/// The guard is one past the ceiling: the filter is allowed to pull the member
/// that discovers exhaustion, but a filter that keeps pulling has lost the
/// bound and must fail the test immediately rather than run to completion.
struct CountedSource {
    /// Members left in this finite source.
    remaining: usize,
    /// Shared count of members pulled so far.
    consumed: Arc<AtomicUsize>,
    /// Highest pull count the filter may reach before this fixture panics.
    guard: usize,
    /// Member value this source yields.
    member: Value,
}

impl Iterator for CountedSource {
    type Item = Value;

    fn next(&mut self) -> Option<Value> {
        if self.remaining == 0 {
            return None;
        }
        let count = self.consumed.fetch_add(1, Ordering::SeqCst) + 1;
        assert!(
            count <= self.guard,
            "compact pulled {count} members, past the {guard}-member guard; the consumption \
             limit did not stop the filter",
            guard = self.guard
        );
        self.remaining -= 1;
        Some(self.member.clone())
    }
}

/// Build a finite iterable that counts its pulls and guards its upper bound.
///
/// The source is finite and several times the ceiling; without the guard a
/// filter that ignored the limit would still terminate, so the guard turns lost
/// bounding into a fast, named failure rather than a slow comparison.
fn counted_source(total: usize, member: Value) -> (Value, Arc<AtomicUsize>) {
    let consumed = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&consumed);
    let source = Value::make_iterable(move || CountedSource {
        remaining: total,
        consumed: Arc::clone(&counter),
        guard: PER_CALL_LIMIT + 1,
        member: member.clone(),
    });
    (source, consumed)
}

/// Render one compact expression under the standalone stdlib environment.
fn compacted_length(
    env: &minijinja::Environment<'_>,
    source: &Value,
) -> Result<usize, minijinja::Error> {
    let expression = env.compile_expression("source | compact | length")?;
    let rendered = expression.eval(context!(source => source))?;
    rendered.as_usize().ok_or_else(|| {
        minijinja::Error::new(
            ErrorKind::InvalidOperation,
            "compact length did not render as a number",
        )
    })
}

/// The reported trigger: a lazy repetition one member over the ceiling fails.
///
/// `[none] * 100001` names 100,001 logical members in a few source bytes. The
/// filter must stop at the ceiling and report the resource limit rather than
/// collect the whole repetition.
#[test]
fn compact_rejects_a_repetition_one_member_over_the_per_call_limit() -> Result<()> {
    let env = fallible::stdlib_env()?;
    let count = PER_CALL_LIMIT + 1;
    let template = format!("{{{{ ([none] * {count}) | compact | length }}}}");
    let Err(error) = env.render_str(&template, context! {}) else {
        bail!("compact must reject a repetition of {count} members");
    };

    ensure!(
        error.kind() == ErrorKind::OutOfFuel,
        "compact should report OutOfFuel, but was {:?}",
        error.kind()
    );
    let message = normalize_fluent_isolates(&error.to_string());
    ensure!(
        message.contains("resource budget exhausted"),
        "the error must report the resource limit: {message}"
    );
    ensure!(
        message.contains("during collection"),
        "the error must name the collection stage: {message}"
    );
    Ok(())
}

/// A repetition exactly at the ceiling is supported.
#[test]
fn compact_accepts_a_repetition_exactly_at_the_per_call_limit() -> Result<()> {
    let env = fallible::stdlib_env()?;
    let count = PER_CALL_LIMIT;
    let template = format!("{{{{ ([none] * {count}) | compact | length }}}}");
    let output = env
        .render_str(&template, context! {})
        .context("a repetition at the per-call ceiling must render")?;

    ensure!(
        output == "0",
        "every member of the all-blank repetition must be dropped, but the surviving length \
         was {output}"
    );
    Ok(())
}

/// An oversized source stops at the ceiling, whatever its members are.
///
/// The blank case is the discriminator for charging before the blank check: a
/// filter that charged only retained members would pull that source to
/// exhaustion without ever charging. The retained case is the discriminator for
/// charging before the append: a filter that appended first would retain one
/// member past the ceiling, which the counted fixture's guard turns into a
/// failure.
///
/// The stage assertion belongs to the blank case because only that case proves
/// the charge happened before the member was classified; the retained case
/// shares the remaining expectations.
#[rstest]
#[case::blank(Value::from(""), true)]
#[case::retained(Value::from("kept"), false)]
fn compact_charges_every_member_before_it_acts(
    #[case] member: Value,
    #[case] checks_stage: bool,
) -> Result<()> {
    let env = fallible::stdlib_env()?;
    let (source, consumed) = counted_source(OVERSIZED_SOURCE, member);

    let error = compacted_length(&env, &source)
        .expect_err("an oversized source must exhaust the per-call ceiling");
    ensure!(
        error.kind() == ErrorKind::OutOfFuel,
        "compact should report OutOfFuel, but was {:?}",
        error.kind()
    );
    if checks_stage {
        ensure!(
            normalize_fluent_isolates(&error.to_string()).contains("during collection"),
            "the error must name the collection stage: {error}"
        );
    }
    ensure!(
        consumed.load(Ordering::SeqCst) == PER_CALL_LIMIT + 1,
        "the filter must stop after the pull that discovers exhaustion, but pulled {} members",
        consumed.load(Ordering::SeqCst)
    );
    Ok(())
}

/// A source below the ceiling is examined in full.
///
/// Every examined member is charged, so a successful call consumes exactly the
/// source's length and retains only its non-blank members.
#[test]
fn compact_examines_and_charges_every_member_of_a_mixed_source() -> Result<()> {
    const TOTAL: usize = 1_000;
    /// Half of [`TOTAL`], written out so the expectation needs no arithmetic.
    const RETAINED: usize = 500;
    let env = fallible::stdlib_env()?;
    let consumed = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&consumed);
    // An explicit alternating flag rather than index arithmetic: the crate
    // denies `%` and `/` on integers, and the intent is "blank, kept, blank,
    // kept" rather than anything about divisibility.
    let source = Value::make_iterable(move || {
        let mut is_blank = true;
        (0..TOTAL)
            .map(|_| {
                counter.fetch_add(1, Ordering::SeqCst);
                let member = if is_blank {
                    Value::from("")
                } else {
                    Value::from("kept")
                };
                is_blank = !is_blank;
                member
            })
            .collect::<Vec<_>>()
            .into_iter()
    });

    let length = compacted_length(&env, &source).context("a mixed source below the ceiling")?;

    ensure!(
        length == RETAINED,
        "compact must retain the non-blank half of a mixed source, but retained {length}"
    );
    ensure!(
        consumed.load(Ordering::SeqCst) == TOTAL,
        "compact must examine every member of a source below the ceiling, but examined {}",
        consumed.load(Ordering::SeqCst)
    );
    Ok(())
}

/// The counted fixture really is an iterable subject.
///
/// `compact` gates on `ValueKind::Seq` and `ValueKind::Iterable`, and the
/// counted tests above would fail for the wrong reason if the fixture were
/// rejected by that gate before the limit was consulted.
#[test]
fn counted_source_is_an_iterable_subject() {
    let (source, _consumed) = counted_source(4, Value::from(""));
    assert_eq!(
        source.kind(),
        ValueKind::Iterable,
        "the counted fixture must reach compact through the iterable arm"
    );
}
