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
fn now_rejects_invalid_offset() {
    let env = build_env();
    let err = env
        .compile_expression("now(offset='bogus')")
        .expect("compile expression")
        .eval(context! {});
    let err = err.expect_err("invalid offset should error");
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
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
fn timedelta_supports_negative_values() {
    let env = build_env();
    let value = eval_expression(&env, "timedelta(hours=-1, seconds=-30)");
    let duration = value_as_duration(&value);
    assert_eq!(duration, Duration::seconds(-SECONDS_PER_HOUR - 30));
}

#[rstest]
fn timedelta_detects_overflow() {
    let env = build_env();
    let err = env
        .compile_expression("timedelta(days=9223372036854775807)")
        .expect("compile expression")
        .eval(context! {});
    let err = err.expect_err("overflow should error");
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
}

#[rstest]
fn timestamp_iso8601_property() {
    let reference = datetime!(2024-05-21 10:30:00 +00:00);
    let value = Value::from_object(TimestampValue::new(reference));
    let object = value.as_object().expect("object");
    let iso = object
        .get_value(&Value::from("iso8601"))
        .expect("iso8601 attr")
        .to_string();
    assert_eq!(iso, "2024-05-21T10:30:00Z");
}

#[rstest]
fn timedelta_iso8601_property() {
    let duration = Duration::seconds(SECONDS_PER_DAY + 30) + Duration::nanoseconds(500_000_000);
    let value = Value::from_object(TimeDeltaValue::new(duration));
    let object = value.as_object().expect("object");
    let iso = object
        .get_value(&Value::from("iso8601"))
        .expect("iso8601 attr")
        .to_string();
    assert_eq!(iso, "P1DT30.5S");
}
