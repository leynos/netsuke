//! Tests for the stdlib time helpers, validating timestamp and duration
//! conversions alongside ISO 8601 formatting. The cases assert that `now`
//! respects UTC defaults, applies caller-provided offsets, rejects malformed
//! offsets, and that helper functions expose consistent object wrappers for
//! downstream template evaluation.
use super::*;
use anyhow::{Context, Result, anyhow, ensure};
use minijinja::{Environment, context, value::Value};
use rstest::{fixture, rstest};
use time::{Duration, OffsetDateTime, UtcOffset, macros::datetime};

fn eval_expression(env: &Environment<'_>, expr: &str) -> Result<Value> {
    let compiled = env.compile_expression(expr)?;
    Ok(compiled.eval(context! {})?)
}

#[fixture]
fn env() -> Environment<'static> {
    let mut env = Environment::new();
    register_functions(&mut env);
    env
}

fn value_as_timestamp(value: &Value) -> Result<OffsetDateTime> {
    value
        .as_object()
        .and_then(|obj| obj.downcast_ref::<TimestampValue>())
        .map(|stored| stored.datetime)
        .ok_or_else(|| anyhow!("value is not a timestamp object"))
}

fn value_as_duration(value: &Value) -> Result<Duration> {
    value
        .as_object()
        .and_then(|obj| obj.downcast_ref::<TimeDeltaValue>())
        .map(|stored| stored.duration)
        .ok_or_else(|| anyhow!("value is not a duration object"))
}

fn get_iso8601_property(value: &Value) -> Result<String> {
    let obj = value.as_object().context("value is not an object")?;
    let iso = obj
        .get_value(&Value::from("iso8601"))
        .context("iso8601 attribute missing")?;
    Ok(iso.to_string())
}

#[rstest]
fn now_defaults_to_utc(env: Environment<'static>) -> Result<()> {
    let value = eval_expression(&env, "now()")?;
    let captured = value_as_timestamp(&value)?;
    let now = OffsetDateTime::now_utc();
    let delta = (now - captured).abs();
    ensure!(delta <= Duration::seconds(2), "delta {delta:?} too large");
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
