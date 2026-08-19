//! Time helpers for the `MiniJinja` standard library.
//!
//! The helpers expose UTC timestamps and duration arithmetic in a deterministic
//! manner so templates can reason about file ages without shelling out. Values
//! round-trip through `MiniJinja` as lightweight objects so other predicates can
//! downcast them later without reparsing strings.

use minijinja::{
    Environment, Error, ErrorKind,
    value::{Kwargs, Value},
};
use time::{
    Duration, OffsetDateTime, UtcOffset, format_description::FormatItem, macros::format_description,
};

use crate::localization::{self, keys};

mod format;
use self::format::{TimeDeltaValue, TimestampValue};

/// Seconds in one minute.
const SECONDS_PER_MINUTE: i64 = 60;
/// Seconds in one hour.
const SECONDS_PER_HOUR: i64 = 60 * SECONDS_PER_MINUTE;
/// Seconds in one day.
const SECONDS_PER_DAY: i64 = 24 * SECONDS_PER_HOUR;
/// Seconds in one week.
const SECONDS_PER_WEEK: i64 = 7 * SECONDS_PER_DAY;
/// Nanoseconds in one microsecond.
const NANOS_PER_MICROSECOND: i64 = 1_000;
/// Nanoseconds in one millisecond.
const NANOS_PER_MILLISECOND: i64 = 1_000 * NANOS_PER_MICROSECOND;
/// Seconds in one minute, in `i32` for offset arithmetic.
const SECONDS_PER_MINUTE_I32: i32 = 60;
/// Seconds in one hour, in `i32` for offset arithmetic.
const SECONDS_PER_HOUR_I32: i32 = 3_600;

/// Format description for an offset's hour, minute, and optional second.
const OFFSET_FMT: &[FormatItem<'static>] =
    format_description!("[offset_hour]:[offset_minute][optional [:[offset_second]]]");

/// Register time helpers with the environment.
pub(crate) fn register_functions(env: &mut Environment<'_>) {
    env.add_function("now", |kwargs: Kwargs| now(&kwargs));
    register_query_functions(env);
}

/// Register time helpers whose output does not depend on the current clock.
pub(crate) fn register_query_functions(env: &mut Environment<'_>) {
    env.add_function("timedelta", |kwargs: Kwargs| timedelta(&kwargs));
}

/// Resolve the `now` helper: the current UTC time shifted by a given offset.
///
/// # Errors
///
/// Returns an invalid-operation error when the offset string does not parse.
fn now(kwargs: &Kwargs) -> Result<Value, Error> {
    let offset_spec: Option<String> = kwargs.get("offset")?;
    kwargs.assert_all_used()?;

    let mut timestamp = OffsetDateTime::now_utc();
    if let Some(raw) = offset_spec {
        let parsed = parse_offset(&raw)?;
        timestamp = timestamp.to_offset(parsed);
    }

    Ok(Value::from_object(TimestampValue::new(timestamp)))
}

/// Parse an offset string into a UTC offset, accepting `Z`/`z` for UTC, then a signed numeric offset.
fn parse_offset(raw: &str) -> Result<UtcOffset, Error> {
    let trimmed = raw.trim();
    if trimmed.eq_ignore_ascii_case("z") {
        return Ok(UtcOffset::UTC);
    }

    if !trimmed.starts_with(['+', '-']) {
        return Err(invalid_offset(raw));
    }

    UtcOffset::parse(trimmed, OFFSET_FMT).map_err(|_| invalid_offset(raw))
}

/// Build the invalid-offset error for the given string.
fn invalid_offset(raw: &str) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        localization::message(keys::STDLIB_TIME_OFFSET_INVALID)
            .with_arg("offset", raw)
            .to_string(),
    )
}

/// The `timedelta` keyword components, in canonical resolution order.
const COMPONENT_SPECS: &[(&str, ComponentSpec)] = &[
    (
        "weeks",
        ComponentSpec {
            multiplier: SECONDS_PER_WEEK,
            constructor: Duration::seconds,
            label_key: keys::STDLIB_TIME_LABEL_WEEKS,
        },
    ),
    (
        "days",
        ComponentSpec {
            multiplier: SECONDS_PER_DAY,
            constructor: Duration::seconds,
            label_key: keys::STDLIB_TIME_LABEL_DAYS,
        },
    ),
    (
        "hours",
        ComponentSpec {
            multiplier: SECONDS_PER_HOUR,
            constructor: Duration::seconds,
            label_key: keys::STDLIB_TIME_LABEL_HOURS,
        },
    ),
    (
        "minutes",
        ComponentSpec {
            multiplier: SECONDS_PER_MINUTE,
            constructor: Duration::seconds,
            label_key: keys::STDLIB_TIME_LABEL_MINUTES,
        },
    ),
    (
        "seconds",
        ComponentSpec {
            multiplier: 1,
            constructor: Duration::seconds,
            label_key: keys::STDLIB_TIME_LABEL_SECONDS,
        },
    ),
    (
        "milliseconds",
        ComponentSpec {
            multiplier: NANOS_PER_MILLISECOND,
            constructor: Duration::nanoseconds,
            label_key: keys::STDLIB_TIME_LABEL_MILLISECONDS,
        },
    ),
    (
        "microseconds",
        ComponentSpec {
            multiplier: NANOS_PER_MICROSECOND,
            constructor: Duration::nanoseconds,
            label_key: keys::STDLIB_TIME_LABEL_MICROSECONDS,
        },
    ),
    (
        "nanoseconds",
        ComponentSpec {
            multiplier: 1,
            constructor: Duration::nanoseconds,
            label_key: keys::STDLIB_TIME_LABEL_NANOSECONDS,
        },
    ),
];

/// One duration component the `timedelta` helper accepts, with its scaling.
#[derive(Clone, Copy)]
struct ComponentSpec {
    /// Multiplier converting the requested unit to the component's basis.
    multiplier: i64,
    /// Constructor turning the scaled value into a [`Duration`].
    constructor: fn(i64) -> Duration,
    /// Fluent key naming the component in overflow diagnostics.
    label_key: &'static str,
}

/// Add one scaled component to `total`, rejecting overflow and `None` amount.
///
/// # Errors
///
/// Returns an invalid-operation error when the scaled component overflows.
fn add_component(
    mut total: Duration,
    amount: Option<i64>,
    spec: ComponentSpec,
) -> Result<Duration, Error> {
    if let Some(value) = amount {
        let scaled = value
            .checked_mul(spec.multiplier)
            .ok_or_else(|| overflow_error(spec.label_key))?;
        let component = (spec.constructor)(scaled);
        total = total
            .checked_add(component)
            .ok_or_else(|| overflow_error(spec.label_key))?;
    }
    Ok(total)
}

/// Build the overflow error for the labelled component.
fn overflow_error(label_key: &'static str) -> Error {
    let component = localization::message(label_key).to_string();
    Error::new(
        ErrorKind::InvalidOperation,
        localization::message(keys::STDLIB_TIME_OVERFLOW)
            .with_arg("component", component)
            .to_string(),
    )
}

/// Resolve the `timedelta` helper from its keyword components.
///
/// # Errors
///
/// Returns an invalid-operation error for unknown component keys or overflow.
fn timedelta(kwargs: &Kwargs) -> Result<Value, Error> {
    let mut total = Duration::ZERO;

    for (name, spec) in COMPONENT_SPECS {
        let amount: Option<i64> = kwargs.get(name)?;
        total = add_component(total, amount, *spec)?;
    }

    kwargs.assert_all_used()?;
    Ok(Value::from_object(TimeDeltaValue::new(total)))
}

#[cfg(test)]
mod tests;
