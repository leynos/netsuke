//! Time helpers for the `MiniJinja` standard library.
//!
//! The helpers expose UTC timestamps and duration arithmetic in a deterministic
//! manner so templates can reason about file ages without shelling out. Values
//! round-trip through `MiniJinja` as lightweight objects so other predicates can
//! downcast them later without reparsing strings.

use std::{fmt, sync::Arc};

use minijinja::{
    Environment, Error, ErrorKind,
    value::{Kwargs, Object, ObjectRepr, Value},
};
use time::{
    Duration, OffsetDateTime, UtcOffset,
    format_description::{FormatItem, well_known::Iso8601},
    macros::format_description,
};

const SECONDS_PER_MINUTE: i64 = 60;
const SECONDS_PER_HOUR: i64 = 60 * SECONDS_PER_MINUTE;
const SECONDS_PER_DAY: i64 = 24 * SECONDS_PER_HOUR;
const SECONDS_PER_WEEK: i64 = 7 * SECONDS_PER_DAY;
const NANOS_PER_MICROSECOND: i64 = 1_000;
const NANOS_PER_MILLISECOND: i64 = 1_000 * NANOS_PER_MICROSECOND;
const SECONDS_PER_MINUTE_I32: i32 = 60;
const SECONDS_PER_HOUR_I32: i32 = 3_600;

const OFFSET_FMT: &[FormatItem<'static>] =
    format_description!("[offset_hour]:[offset_minute][optional [:[offset_second]]]");

/// Register time helpers with the environment.
pub(crate) fn register_functions(env: &mut Environment<'_>) {
    env.add_function("now", |kwargs: Kwargs| now(&kwargs));
    env.add_function("timedelta", |kwargs: Kwargs| timedelta(&kwargs));
}

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

fn invalid_offset(raw: &str) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!("now offset '{raw}' is invalid: expected '+HH:MM[:SS]' or 'Z'"),
    )
}

const COMPONENT_SPECS: &[(&str, ComponentSpec)] = &[
    (
        "weeks",
        ComponentSpec {
            multiplier: SECONDS_PER_WEEK,
            constructor: Duration::seconds,
            label: "weeks",
        },
    ),
    (
        "days",
        ComponentSpec {
            multiplier: SECONDS_PER_DAY,
            constructor: Duration::seconds,
            label: "days",
        },
    ),
    (
        "hours",
        ComponentSpec {
            multiplier: SECONDS_PER_HOUR,
            constructor: Duration::seconds,
            label: "hours",
        },
    ),
    (
        "minutes",
        ComponentSpec {
            multiplier: SECONDS_PER_MINUTE,
            constructor: Duration::seconds,
            label: "minutes",
        },
    ),
    (
        "seconds",
        ComponentSpec {
            multiplier: 1,
            constructor: Duration::seconds,
            label: "seconds",
        },
    ),
    (
        "milliseconds",
        ComponentSpec {
            multiplier: NANOS_PER_MILLISECOND,
            constructor: Duration::nanoseconds,
            label: "milliseconds",
        },
    ),
    (
        "microseconds",
        ComponentSpec {
            multiplier: NANOS_PER_MICROSECOND,
            constructor: Duration::nanoseconds,
            label: "microseconds",
        },
    ),
    (
        "nanoseconds",
        ComponentSpec {
            multiplier: 1,
            constructor: Duration::nanoseconds,
            label: "nanoseconds",
        },
    ),
];

#[derive(Clone, Copy)]
struct ComponentSpec {
    multiplier: i64,
    constructor: fn(i64) -> Duration,
    label: &'static str,
}

fn add_component(
    mut total: Duration,
    amount: Option<i64>,
    spec: ComponentSpec,
) -> Result<Duration, Error> {
    if let Some(value) = amount {
        let scaled = value
            .checked_mul(spec.multiplier)
            .ok_or_else(|| overflow_error(spec.label))?;
        let component = (spec.constructor)(scaled);
        total = total
            .checked_add(component)
            .ok_or_else(|| overflow_error(spec.label))?;
    }
    Ok(total)
}

fn overflow_error(label: &str) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!("timedelta overflow when adding {label}"),
    )
}

fn timedelta(kwargs: &Kwargs) -> Result<Value, Error> {
    let mut total = Duration::ZERO;

    for (name, spec) in COMPONENT_SPECS {
        let amount: Option<i64> = kwargs.get(name)?;
        total = add_component(total, amount, *spec)?;
    }

    kwargs.assert_all_used()?;
    Ok(Value::from_object(TimeDeltaValue::new(total)))
}

fn has_timezone_after(formatted: &str, pos: usize) -> bool {
    formatted
        .get(pos + 10..)
        .and_then(|rest| rest.chars().next())
        .is_some_and(|next| matches!(next, 'Z' | '+' | '-'))
}

fn format_offset_datetime(datetime: OffsetDateTime) -> String {
    datetime.format(&Iso8601::DEFAULT).map_or_else(
        |_| datetime.to_string(),
        |mut formatted| {
            if let Some(pos) = formatted.find(".000000000")
                && has_timezone_after(&formatted, pos)
            {
                formatted.replace_range(pos..pos + 10, "");
            }
            formatted
        },
    )
}

fn format_utc_offset(offset: UtcOffset) -> String {
    let total_seconds = offset.whole_seconds();
    let sign = if total_seconds >= 0 { '+' } else { '-' };
    let abs_seconds = total_seconds.abs();
    let hours = abs_seconds.div_euclid(SECONDS_PER_HOUR_I32);
    let remainder = abs_seconds.rem_euclid(SECONDS_PER_HOUR_I32);
    let minutes = remainder.div_euclid(SECONDS_PER_MINUTE_I32);
    let seconds = remainder.rem_euclid(SECONDS_PER_MINUTE_I32);

    if seconds == 0 {
        format!("{sign}{hours:02}:{minutes:02}")
    } else {
        format!("{sign}{hours:02}:{minutes:02}:{seconds:02}")
    }
}

fn format_duration_iso8601(duration: Duration) -> String {
    if duration.is_zero() {
        return "PT0S".to_owned();
    }

    let mut buffer = String::new();
    if duration.is_negative() {
        buffer.push('-');
    }
    buffer.push('P');

    let absolute = duration.abs();
    let days = absolute.whole_days();
    let remainder = absolute - Duration::days(days);

    if days != 0 {
        buffer.push_str(&days.to_string());
        buffer.push('D');
    }

    let time_section = format_time_components(remainder);
    finalize_duration_buffer(buffer, &time_section)
}

fn format_time_components(mut remainder: Duration) -> String {
    let mut time_section = String::new();

    let hours = remainder.whole_hours();
    if hours != 0 {
        time_section.push_str(&hours.to_string());
        time_section.push('H');
        remainder -= Duration::hours(hours);
    }

    let minutes = remainder.whole_minutes();
    if minutes != 0 {
        time_section.push_str(&minutes.to_string());
        time_section.push('M');
        remainder -= Duration::minutes(minutes);
    }

    let seconds = remainder.whole_seconds();
    let nanos = remainder.subsec_nanoseconds();
    if seconds != 0 || nanos != 0 {
        time_section.push_str(&format_seconds_with_fraction(seconds, nanos));
    }

    time_section
}

fn finalize_duration_buffer(mut buffer: String, time_section: &str) -> String {
    if time_section.is_empty() {
        if buffer.ends_with('P') {
            buffer.push_str("T0S");
        }
    } else {
        buffer.push('T');
        buffer.push_str(time_section);
    }

    buffer
}

fn format_seconds_with_fraction(seconds: i64, nanos: i32) -> String {
    let seconds_u64 = u64::try_from(seconds)
        .expect("seconds must be non-negative when formatting from absolute duration remainder");
    if nanos == 0 {
        return format!("{seconds_u64}S");
    }

    let mut fraction = format!("{nanos:09}");
    while fraction.ends_with('0') {
        fraction.pop();
    }

    format!("{seconds_u64}.{fraction}S")
}

#[derive(Clone, Copy)]
struct TimestampValue {
    datetime: OffsetDateTime,
}

impl TimestampValue {
    fn new(datetime: OffsetDateTime) -> Self {
        Self { datetime }
    }

    fn iso8601(&self) -> String {
        format_offset_datetime(self.datetime)
    }
}

impl fmt::Debug for TimestampValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.iso8601())
    }
}

impl Object for TimestampValue {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Plain
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        let attr = key.as_str()?;
        match attr {
            "iso8601" => Some(Value::from(self.iso8601())),
            "unix_timestamp" => Some(Value::from(self.datetime.unix_timestamp())),
            "offset" => Some(Value::from(format_utc_offset(self.datetime.offset()))),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
struct TimeDeltaValue {
    duration: Duration,
}

impl TimeDeltaValue {
    fn new(duration: Duration) -> Self {
        Self { duration }
    }

    fn iso8601(&self) -> String {
        format_duration_iso8601(self.duration)
    }
}

impl fmt::Debug for TimeDeltaValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.iso8601())
    }
}

impl Object for TimeDeltaValue {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Plain
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        let attr = key.as_str()?;
        match attr {
            "iso8601" => Some(Value::from(self.iso8601())),
            "seconds" => Some(Value::from(self.duration.whole_seconds())),
            "nanoseconds" => Some(Value::from(i64::from(self.duration.subsec_nanoseconds()))),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
