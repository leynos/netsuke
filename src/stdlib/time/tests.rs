//! Tests for the stdlib time helpers, validating timestamp and duration
//! conversions alongside ISO 8601 formatting. The cases assert that `now`
//! respects UTC defaults, applies caller-provided offsets, rejects malformed
//! offsets, and that helper functions expose consistent object wrappers for
//! downstream template evaluation.
use super::*;
use anyhow::{Context, Result, anyhow, bail, ensure};
use googletest::prelude::*;
use minijinja::{Environment, ErrorKind, context, value::Value};
use proptest::prelude::*;
use rstest::{fixture, rstest};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use time::{Duration, OffsetDateTime, UtcOffset, macros::datetime};

fn eval_expression(env: &Environment<'_>, expr: &str) -> Result<Value> {
    let compiled = env
        .compile_expression(expr)
        .with_context(|| format!("compiling expression: {expr}"))?;
    compiled
        .eval(context! {})
        .with_context(|| format!("evaluating expression: {expr}"))
}

#[fixture]
fn env() -> Environment<'static> {
    let mut env = Environment::new();
    register_functions(&mut env, WallClock::default());
    env
}

/// Build an environment whose `now()` always reports `instant`.
fn env_with_fixed_clock(instant: OffsetDateTime) -> Environment<'static> {
    let mut env = Environment::new();
    register_functions(&mut env, WallClock::new(fixed_clock(instant)));
    env
}

/// Build an environment whose `now()` reports `first`, then each element of
/// `rest` in turn, saturating at the last, and recording how many times the
/// provider was consulted.
///
/// Taking `first` separately makes non-emptiness a type-level precondition.
fn env_with_sequenced_clock(
    first: OffsetDateTime,
    rest: &[OffsetDateTime],
) -> (Environment<'static>, Arc<AtomicUsize>) {
    let mut instants = vec![first];
    instants.extend_from_slice(rest);
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    let provider: ClockProvider = Arc::new(move || {
        let index = counter.fetch_add(1, Ordering::SeqCst);
        instants
            .get(index)
            .or_else(|| instants.last())
            .copied()
            .unwrap_or(first)
    });
    let mut env = Environment::new();
    register_functions(&mut env, WallClock::new(provider));
    (env, calls)
}

fn value_as_timestamp(value: &Value) -> Result<OffsetDateTime> {
    value
        .as_object()
        .and_then(|obj| obj.downcast_ref::<TimestampValue>())
        .map(|stored| stored.datetime)
        .ok_or_else(|| anyhow!("value is not a timestamp object: {value:?}"))
}

fn value_as_duration(value: &Value) -> Result<Duration> {
    value
        .as_object()
        .and_then(|obj| obj.downcast_ref::<TimeDeltaValue>())
        .map(|stored| stored.duration)
        .ok_or_else(|| anyhow!("value is not a duration object: {value:?}"))
}

fn get_iso8601_property(value: &Value) -> Result<String> {
    let obj = value.as_object().context("value is not an object")?;
    let iso = obj
        .get_value(&Value::from("iso8601"))
        .context("iso8601 attribute missing")?;
    iso.as_str()
        .map(ToOwned::to_owned)
        .context("iso8601 attribute is not a string")
}

#[rstest]
fn now_defaults_to_utc(env: Environment<'static>) -> Result<()> {
    let value = eval_expression(&env, "now()")?;
    let captured = value_as_timestamp(&value)?;
    let now = OffsetDateTime::now_utc();
    let delta = (now - captured).abs();
    ensure!(delta <= Duration::seconds(3), "delta {delta:?} too large");
    ensure!(captured.offset() == UtcOffset::UTC);
    Ok(())
}

#[rstest]
fn now_applies_custom_offset(env: Environment<'static>) -> Result<()> {
    let value = eval_expression(&env, "now(offset='+02:30')")?;
    let captured = value_as_timestamp(&value)?;
    let offset = UtcOffset::from_hms(2, 30, 0)?;
    ensure!(captured.offset() == offset);
    Ok(())
}

/// Accept upper- and lower-case UTC shorthand offsets.
#[rstest]
#[case::uppercase("Z")]
#[case::lowercase("z")]
fn now_accepts_utc_shorthand(env: Environment<'static>, #[case] offset: &str) -> Result<()> {
    let value = eval_expression(&env, &format!("now(offset='{offset}')"))?;
    let captured = value_as_timestamp(&value)?;
    ensure!(captured.offset() == UtcOffset::UTC);
    Ok(())
}

#[rstest]
#[case::nonsense("bogus")]
#[case::missing_sign("01:00")]
#[case::hours_out_of_range("+25:00")]
#[case::minutes_out_of_range("+01:60")]
#[case::seconds_out_of_range("+01:01:61")]
#[case::empty("")]
fn now_rejects_invalid_offset(env: Environment<'static>, #[case] offset: &str) -> Result<()> {
    let expr = format!("now(offset='{offset}')");
    let compiled = env.compile_expression(&expr)?;
    match compiled.eval(context! {}) {
        Ok(value) => Err(anyhow!("expected invalid offset to fail, got {value:?}")),
        Err(err) => {
            ensure!(err.kind() == ErrorKind::InvalidOperation);
            Ok(())
        }
    }
}

/// An injected instant is reported verbatim, whatever offset its provider
/// carries. The non-UTC case is load-bearing: [`WallClock::read`] normalizes
/// to UTC, and without that normalization a fixture built from a local-time
/// literal would make `now()` render a timestamp production never produces.
#[rstest]
#[case::utc(datetime!(2026-06-08 12:00:00 UTC))]
#[case::epoch(datetime!(1970-01-01 00:00:00 UTC))]
#[case::far_future(datetime!(2099-12-31 23:59:59 UTC))]
#[case::non_utc(datetime!(2026-06-08 17:30:00 +05:30))]
fn now_uses_injected_clock(#[case] instant: OffsetDateTime) -> Result<()> {
    let env = env_with_fixed_clock(instant);
    let value = eval_expression(&env, "now()")?;
    let captured = value_as_timestamp(&value)?;

    assert_that!(captured, eq(instant));
    assert_that!(captured.offset(), eq(UtcOffset::UTC));
    Ok(())
}

/// Two evaluations under one fixed provider agree, including two calls within
/// a single expression — the property a manifest author actually relies on.
#[rstest]
fn now_repeats_the_injected_instant() -> Result<()> {
    let fixed = datetime!(2026-06-08 12:00:00 UTC);
    let env = env_with_fixed_clock(fixed);

    let first = value_as_timestamp(&eval_expression(&env, "now()")?)?;
    let second = value_as_timestamp(&eval_expression(&env, "now()")?)?;
    assert_that!(first, eq(fixed));
    assert_that!(second, eq(fixed));

    let joined = eval_expression(&env, "now().iso8601 ~ '|' ~ now().iso8601")?;
    assert_that!(
        joined.as_str(),
        eq(Some("2026-06-08T12:00:00Z|2026-06-08T12:00:00Z"))
    );
    Ok(())
}

/// The provider is consulted afresh on every call rather than read once while
/// the registered closure is built. A fixed provider cannot distinguish the
/// two, so the negative control needs a provider whose output varies.
#[rstest]
fn now_reads_the_provider_on_every_call() -> Result<()> {
    let (env, _calls) = env_with_sequenced_clock(
        datetime!(2026-06-08 12:00:00 UTC),
        &[
            datetime!(2026-06-08 13:00:00 UTC),
            datetime!(2026-06-08 14:00:00 UTC),
        ],
    );

    let first = value_as_timestamp(&eval_expression(&env, "now()")?)?;
    let second = value_as_timestamp(&eval_expression(&env, "now()")?)?;
    let third = value_as_timestamp(&eval_expression(&env, "now()")?)?;

    assert_that!(first, eq(datetime!(2026-06-08 12:00:00 UTC)));
    assert_that!(second, eq(datetime!(2026-06-08 13:00:00 UTC)));
    assert_that!(third, eq(datetime!(2026-06-08 14:00:00 UTC)));
    Ok(())
}

/// Each `now()` evaluation consults the provider exactly once. The expected
/// count is derived from the evaluations performed, so a re-reading
/// implementation fails on the count rather than on a stale literal.
#[rstest]
fn now_invokes_the_provider_once_per_call() -> Result<()> {
    let (env, calls) = env_with_sequenced_clock(
        datetime!(2026-06-08 12:00:00 UTC),
        &[datetime!(2026-06-08 13:00:00 UTC)],
    );

    let expressions = ["now()", "now(offset='+02:00')"];
    for expression in expressions {
        eval_expression(&env, expression)?;
    }

    assert_that!(calls.load(Ordering::SeqCst), eq(expressions.len()));
    Ok(())
}

/// Applying an offset re-expresses the injected instant, preserving it. Both
/// conjuncts are load-bearing: an arithmetic shift keeps the offset but moves
/// the instant, and dropping the offset keeps the instant but loses the
/// requested representation.
#[rstest]
#[case("Z")]
#[case("+00:00")]
#[case("+02:30")]
#[case("-05:00")]
#[case("+23:59:59")]
#[case("-23:59:59")]
fn now_applies_offset_to_the_injected_instant(#[case] offset_spec: &str) -> Result<()> {
    let fixed = datetime!(2026-06-08 12:00:00 UTC);
    let env = env_with_fixed_clock(fixed);
    let value = eval_expression(&env, &format!("now(offset='{offset_spec}')"))?;
    let captured = value_as_timestamp(&value)?;
    let expected_offset = parse_offset(offset_spec)?;

    assert_that!(captured.unix_timestamp(), eq(fixed.unix_timestamp()));
    assert_that!(captured.offset(), eq(expected_offset));
    Ok(())
}

proptest! {
    /// Re-expressing the injected instant in any valid offset preserves it.
    #[test]
    fn now_offset_preserves_the_instant(
        sign in prop_oneof![Just("+"), Just("-")],
        hour in 0_u8..24,
        minute in 0_u8..60,
        second in 0_u8..60,
    ) {
        let offset = format!("{sign}{hour:02}:{minute:02}:{second:02}");
        let fixed = datetime!(2026-06-08 12:00:00 UTC);
        let env = env_with_fixed_clock(fixed);
        let expression = format!("now(offset='{offset}')");
        let value = eval_expression(&env, &expression)
            .map_err(|err| TestCaseError::fail(format!("evaluating {expression}: {err}")))?;
        let captured = value_as_timestamp(&value)
            .map_err(|err| TestCaseError::fail(format!("reading {expression}: {err}")))?;
        let expected_offset = parse_offset(&offset)
            .map_err(|err| TestCaseError::fail(format!("parsing {offset}: {err}")))?;

        prop_assert_eq!(captured.unix_timestamp(), fixed.unix_timestamp());
        prop_assert_eq!(captured.offset(), expected_offset);
    }
}

proptest! {
    /// Accept signed offsets whose hour component stays within one civil day.
    #[test]
    fn parse_offset_accepts_hours_below_a_civil_day(
        sign in prop_oneof![Just("+"), Just("-")],
        hour in 0_u8..24,
        minute in 0_u8..60,
        second in 0_u8..60,
    ) {
        let offset = format!("{sign}{hour:02}:{minute:02}:{second:02}");
        prop_assert!(parse_offset(&offset).is_ok(), "expected {offset} to be accepted");
    }

    /// Reject signed offsets at or beyond the one-civil-day boundary.
    #[test]
    fn parse_offset_rejects_a_civil_day_or_more(
        sign in prop_oneof![Just("+"), Just("-")],
        hour in 24_u8..=48,
        minute in 0_u8..60,
        second in 0_u8..60,
    ) {
        let offset = format!("{sign}{hour:02}:{minute:02}:{second:02}");
        prop_assert!(parse_offset(&offset).is_err(), "expected {offset} to be rejected");
    }
}

/// Evaluate `expr`, returning the raw `MiniJinja` error it raised.
///
/// The typed error is returned rather than an `anyhow` one because the
/// manifest-query marker is inspected through `minijinja::Error`; wrapping it
/// first would hide the type the assertion needs.
fn eval_expression_error(env: &Environment<'_>, expr: &str) -> Result<minijinja::Error> {
    let compiled = env
        .compile_expression(expr)
        .with_context(|| format!("compiling expression: {expr}"))?;
    match compiled.eval(context! {}) {
        Ok(value) => bail!("expected {expr} to fail, but it produced {value:?}"),
        Err(error) => Ok(error),
    }
}

/// Constraint C2: manifest-query registration must keep refusing `now()`, so
/// discovery metadata can never disclose host time. The refusal is asserted to
/// name `now` rather than merely carry the marker, which rejects a copy-pasted
/// stub registered under the wrong helper name.
#[rstest]
#[case::bare("now()")]
#[case::offset("now(offset='+02:00')")]
fn manifest_query_registration_refuses_now(#[case] expression: &str) -> Result<()> {
    let mut env = Environment::new();
    let _state = crate::stdlib::register_manifest_query(&mut env);

    let error = eval_expression_error(&env, expression)?;

    assert_that!(
        crate::stdlib::is_manifest_query_disabled_error(&error),
        eq(true)
    );
    ensure!(
        error
            .detail()
            .unwrap_or_default()
            .starts_with("now is disabled while rendering"),
        "refusal should name the `now` helper: {error}"
    );
    Ok(())
}

/// The permissive half of query registration stays clock-free: it does not
/// define `now` at all. This is the sharper half of C2 — a helper that was
/// never declared cannot acquire a clock by having one threaded into it — and
/// it is distinguished from the refusal above by the error kind rather than by
/// the mere absence of a value.
#[rstest]
#[case::bare("now()")]
#[case::offset("now(offset='+02:00')")]
fn query_functions_do_not_define_now(#[case] expression: &str) -> Result<()> {
    let mut env = Environment::new();
    register_query_functions(&mut env);

    let error = eval_expression_error(&env, expression)?;

    assert_that!(error.kind(), eq(ErrorKind::UnknownFunction));
    assert_that!(
        crate::stdlib::is_manifest_query_disabled_error(&error),
        eq(false)
    );
    Ok(())
}

#[rstest]
fn timedelta_defaults_to_zero(env: Environment<'static>) -> Result<()> {
    let value = eval_expression(&env, "timedelta()")?;
    let duration = value_as_duration(&value)?;
    ensure!(duration.is_zero(), "duration {duration:?} should be zero");
    Ok(())
}

#[rstest]
fn timedelta_accumulates_components(env: Environment<'static>) -> Result<()> {
    let value = eval_expression(
        &env,
        "timedelta(days=1, hours=2, minutes=30, seconds=5, milliseconds=750, microseconds=250, nanoseconds=1)",
    )?;
    let duration = value_as_duration(&value)?;
    let expected = Duration::seconds(SECONDS_PER_DAY)
        + Duration::seconds(SECONDS_PER_HOUR * 2)
        + Duration::seconds(SECONDS_PER_MINUTE * 30)
        + Duration::seconds(5)
        + Duration::nanoseconds(750 * NANOS_PER_MILLISECOND)
        + Duration::nanoseconds(250 * NANOS_PER_MICROSECOND)
        + Duration::nanoseconds(1);
    ensure!(duration == expected);
    Ok(())
}

#[rstest]
#[case("timedelta(weeks=-1)", Duration::seconds(-SECONDS_PER_WEEK))]
#[case("timedelta(days=-1)", Duration::seconds(-SECONDS_PER_DAY))]
#[case("timedelta(hours=-1)", Duration::seconds(-SECONDS_PER_HOUR))]
#[case("timedelta(minutes=-1)", Duration::seconds(-SECONDS_PER_MINUTE))]
#[case("timedelta(seconds=-1)", Duration::seconds(-1))]
#[case(
    "timedelta(milliseconds=-1)",
    Duration::nanoseconds(-NANOS_PER_MILLISECOND),
)]
#[case(
    "timedelta(microseconds=-1)",
    Duration::nanoseconds(-NANOS_PER_MICROSECOND),
)]
#[case("timedelta(nanoseconds=-1)", Duration::nanoseconds(-1))]
fn timedelta_supports_negative_values(
    env: Environment<'static>,
    #[case] expr: &str,
    #[case] expected: Duration,
) -> Result<()> {
    let value = eval_expression(&env, expr)?;
    let duration = value_as_duration(&value)?;
    ensure!(duration == expected);
    Ok(())
}

#[rstest]
fn timedelta_detects_overflow(env: Environment<'static>) -> Result<()> {
    let compiled = env.compile_expression("timedelta(days=9223372036854775807)")?;
    match compiled.eval(context! {}) {
        Ok(value) => Err(anyhow!("expected overflow but evaluated to {value:?}")),
        Err(err) => {
            ensure!(err.kind() == ErrorKind::InvalidOperation);
            Ok(())
        }
    }
}

#[rstest]
#[case(
    datetime!(2024-05-21 10:30:00 +00:00),
    "2024-05-21T10:30:00Z",
)]
#[case(
    datetime!(2024-05-21 10:30:00 +05:45),
    "2024-05-21T10:30:00+05:45",
)]
#[case(
    datetime!(2024-05-21 10:30:00.123456789 +00:00),
    "2024-05-21T10:30:00.123456789Z",
)]
#[case(
    datetime!(2024-05-21 10:30:00.5 -03:30),
    "2024-05-21T10:30:00.500000000-03:30",
)]
fn timestamp_iso8601_property(
    #[case] reference: OffsetDateTime,
    #[case] expected: &str,
) -> Result<()> {
    let value = Value::from_object(TimestampValue::new(reference));
    let iso = get_iso8601_property(&value)?;
    ensure!(iso == expected);
    Ok(())
}

#[rstest]
#[case(
    Duration::seconds(SECONDS_PER_DAY + 30) + Duration::nanoseconds(500_000_000),
    "P1DT30.5S",
)]
#[case(Duration::ZERO, "PT0S")]
#[case(
    Duration::seconds(-SECONDS_PER_DAY - 90),
    "-P1DT1M30S",
)]
#[case(
    Duration::seconds(-30) + Duration::nanoseconds(-250_000_000),
    "-PT30.25S",
)]
fn timedelta_iso8601_property(#[case] duration: Duration, #[case] expected: &str) -> Result<()> {
    let value = Value::from_object(TimeDeltaValue::new(duration));
    let iso = get_iso8601_property(&value)?;
    ensure!(iso == expected);
    Ok(())
}
