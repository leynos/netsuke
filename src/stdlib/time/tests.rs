#![allow(
    clippy::expect_used,
    reason = "time tests prefer expect for concise assertions"
)]

use super::*;
use minijinja::{Environment, context, value::Value};
use rstest::rstest;
use time::{Duration, OffsetDateTime, macros::datetime};

fn eval_expression(env: &Environment<'_>, expr: &str) -> Value {
    env.compile_expression(expr)
        .expect("compile expression")
        .eval(context! {})
        .expect("evaluate expression")
}

fn build_env() -> Environment<'static> {
    let mut env = Environment::new();
    register_functions(&mut env);
    env
}

fn value_as_timestamp(value: &Value) -> OffsetDateTime {
    value
        .as_object()
        .and_then(|obj| obj.downcast_ref::<TimestampValue>())
        .map(|stored| stored.datetime)
        .expect("timestamp object")
}

fn value_as_duration(value: &Value) -> Duration {
    value
        .as_object()
        .and_then(|obj| obj.downcast_ref::<TimeDeltaValue>())
        .map(|stored| stored.duration)
        .expect("duration object")
}

fn get_iso8601_property(value: &Value) -> String {
    value
        .as_object()
        .expect("object")
        .get_value(&Value::from("iso8601"))
        .expect("iso8601 attr")
        .to_string()
}

#[rstest]
fn now_defaults_to_utc() {
    let env = build_env();
    let value = eval_expression(&env, "now()");
    let captured = value_as_timestamp(&value);
    let now = OffsetDateTime::now_utc();
    let delta = (now - captured).abs();
    assert!(delta <= Duration::seconds(2));
    assert_eq!(captured.offset(), UtcOffset::UTC);
}

#[rstest]
fn now_applies_custom_offset() {
    let env = build_env();
    let value = eval_expression(&env, "now(offset='+02:30')");
    let captured = value_as_timestamp(&value);
    let offset = UtcOffset::from_hms(2, 30, 0).expect("offset");
    assert_eq!(captured.offset(), offset);
}

#[rstest]
#[case::nonsense("bogus")]
#[case::missing_sign("01:00")]
#[case::hours_out_of_range("+25:00")]
#[case::minutes_out_of_range("+01:60")]
#[case::seconds_out_of_range("+01:01:61")]
#[case::empty("")]
fn now_rejects_invalid_offset(#[case] offset: &str) {
    let env = build_env();
    let expr = format!("now(offset='{offset}')");
    let eval_result = env
        .compile_expression(&expr)
        .expect("compile expression")
        .eval(context! {});
    let evaluation_error = eval_result.expect_err("invalid offset should error");
    assert_eq!(evaluation_error.kind(), ErrorKind::InvalidOperation);
}

#[rstest]
fn timedelta_defaults_to_zero() {
    let env = build_env();
    let value = eval_expression(&env, "timedelta()");
    let duration = value_as_duration(&value);
    assert!(duration.is_zero());
}

#[rstest]
fn timedelta_accumulates_components() {
    let env = build_env();
    let value = eval_expression(
        &env,
        "timedelta(days=1, hours=2, minutes=30, seconds=5, milliseconds=750, microseconds=250, nanoseconds=1)",
    );
    let duration = value_as_duration(&value);
    let expected = Duration::seconds(SECONDS_PER_DAY)
        + Duration::seconds(SECONDS_PER_HOUR * 2)
        + Duration::seconds(SECONDS_PER_MINUTE * 30)
        + Duration::seconds(5)
        + Duration::nanoseconds(750 * NANOS_PER_MILLISECOND)
        + Duration::nanoseconds(250 * NANOS_PER_MICROSECOND)
        + Duration::nanoseconds(1);
    assert_eq!(duration, expected);
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
fn timedelta_supports_negative_values(#[case] expr: &str, #[case] expected: Duration) {
    let env = build_env();
    let value = eval_expression(&env, expr);
    let duration = value_as_duration(&value);
    assert_eq!(duration, expected);
}

#[rstest]
fn timedelta_detects_overflow() {
    let env = build_env();
    let eval_result = env
        .compile_expression("timedelta(days=9223372036854775807)")
        .expect("compile expression")
        .eval(context! {});
    let evaluation_error = eval_result.expect_err("overflow should error");
    assert_eq!(evaluation_error.kind(), ErrorKind::InvalidOperation);
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
fn timestamp_iso8601_property(#[case] reference: OffsetDateTime, #[case] expected: &str) {
    let value = Value::from_object(TimestampValue::new(reference));
    let iso = get_iso8601_property(&value);
    assert_eq!(iso, expected);
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
fn timedelta_iso8601_property(#[case] duration: Duration, #[case] expected: &str) {
    let value = Value::from_object(TimeDeltaValue::new(duration));
    let iso = get_iso8601_property(&value);
    assert_eq!(iso, expected);
}
