//! Shared fixtures and helpers for the stdlib time tests.
//!
//! Evaluation and value-inspection boilerplate lives here so the behaviour
//! tests in the `tests` module and the clock-seam tests in the `clock_tests`
//! module can both stay focused on what they assert.

use super::*;
use anyhow::{Context, Result, anyhow};
use minijinja::{Environment, context, value::Value};
use rstest::fixture;
use time::{Duration, OffsetDateTime};

/// Evaluate `expr` against `env`, contextualizing compile and run errors.
pub(super) fn eval_expression(env: &Environment<'_>, expr: &str) -> Result<Value> {
    let compiled = env
        .compile_expression(expr)
        .with_context(|| format!("compiling expression: {expr}"))?;
    compiled
        .eval(context! {})
        .with_context(|| format!("evaluating expression: {expr}"))
}

/// Build an environment whose `now()` reads the ambient host clock.
#[fixture]
pub(super) fn env() -> Environment<'static> {
    let mut env = Environment::new();
    register_functions(&mut env, WallClock::default());
    env
}

/// Downcast `value` to the timestamp object `now()` produces.
pub(super) fn value_as_timestamp(value: &Value) -> Result<OffsetDateTime> {
    value
        .as_object()
        .and_then(|obj| obj.downcast_ref::<TimestampValue>())
        .map(|stored| stored.datetime)
        .ok_or_else(|| anyhow!("value is not a timestamp object: {value:?}"))
}

/// Downcast `value` to the duration object `timedelta()` produces.
pub(super) fn value_as_duration(value: &Value) -> Result<Duration> {
    value
        .as_object()
        .and_then(|obj| obj.downcast_ref::<TimeDeltaValue>())
        .map(|stored| stored.duration)
        .ok_or_else(|| anyhow!("value is not a duration object: {value:?}"))
}

/// Read the `iso8601` attribute of `value` as a string.
pub(super) fn get_iso8601_property(value: &Value) -> Result<String> {
    let obj = value.as_object().context("value is not an object")?;
    let iso = obj
        .get_value(&Value::from("iso8601"))
        .context("iso8601 attribute missing")?;
    iso.as_str()
        .map(ToOwned::to_owned)
        .context("iso8601 attribute is not a string")
}
