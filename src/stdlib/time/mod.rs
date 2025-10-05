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
use time::{Duration, OffsetDateTime, UtcOffset, format_description::well_known::Iso8601};

const SECONDS_PER_MINUTE: i64 = 60;
const SECONDS_PER_HOUR: i64 = 60 * SECONDS_PER_MINUTE;
const SECONDS_PER_DAY: i64 = 24 * SECONDS_PER_HOUR;
const SECONDS_PER_WEEK: i64 = 7 * SECONDS_PER_DAY;
const NANOS_PER_MICROSECOND: i64 = 1_000;
const NANOS_PER_MILLISECOND: i64 = 1_000 * NANOS_PER_MICROSECOND;
const SECONDS_PER_MINUTE_I32: i32 = 60;
const SECONDS_PER_HOUR_I32: i32 = 3_600;

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

fn timedelta(kwargs: &Kwargs) -> Result<Value, Error> {
    let weeks: Option<i64> = kwargs.get("weeks")?;
    let days: Option<i64> = kwargs.get("days")?;
    let hours: Option<i64> = kwargs.get("hours")?;
    let minutes: Option<i64> = kwargs.get("minutes")?;
    let seconds: Option<i64> = kwargs.get("seconds")?;
    let milliseconds: Option<i64> = kwargs.get("milliseconds")?;
    let microseconds: Option<i64> = kwargs.get("microseconds")?;
    let nanoseconds: Option<i64> = kwargs.get("nanoseconds")?;
    kwargs.assert_all_used()?;

    let mut total = Duration::ZERO;
    total = add_seconds_component(total, weeks, SECONDS_PER_WEEK, "weeks")?;
    total = add_seconds_component(total, days, SECONDS_PER_DAY, "days")?;
    total = add_seconds_component(total, hours, SECONDS_PER_HOUR, "hours")?;
    total = add_seconds_component(total, minutes, SECONDS_PER_MINUTE, "minutes")?;
    total = add_seconds_component(total, seconds, 1, "seconds")?;
    total = add_nanoseconds_component(total, milliseconds, NANOS_PER_MILLISECOND, "milliseconds")?;
    total = add_nanoseconds_component(total, microseconds, NANOS_PER_MICROSECOND, "microseconds")?;
    total = add_nanoseconds_component(total, nanoseconds, 1, "nanoseconds")?;

    Ok(Value::from_object(TimeDeltaValue::new(total)))
}

fn parse_offset(raw: &str) -> Result<UtcOffset, Error> {
    let trimmed = raw.trim();
    if trimmed.eq_ignore_ascii_case("z") {
        return Ok(UtcOffset::UTC);
    }

    let (sign, rest) = if let Some(remaining) = trimmed.strip_prefix('+') {
        (1_i64, remaining)
    } else if let Some(remaining) = trimmed.strip_prefix('-') {
        (-1_i64, remaining)
    } else {
        return Err(invalid_offset(raw));
    };

    let (hours_part, remaining) = rest.split_once(':').ok_or_else(|| invalid_offset(raw))?;
    if hours_part.contains(':') {
        return Err(invalid_offset(raw));
    }

    let (minutes_part, seconds_part) = match remaining.split_once(':') {
        Some((mins, secs)) if !secs.contains(':') => (mins, Some(secs)),
        Some(_) => return Err(invalid_offset(raw)),
        None => (remaining, None),
    };

    if minutes_part.contains(':') {
        return Err(invalid_offset(raw));
    }

    let hours = parse_component(hours_part, raw)?;
    let minutes = parse_component(minutes_part, raw)?;
    let seconds = seconds_part
        .map(|value| parse_component(value, raw))
        .transpose()?;

    if !(0..=23).contains(&hours) || !(0..=59).contains(&minutes) {
        return Err(invalid_offset(raw));
    }

    let seconds_value = seconds.unwrap_or_default();
    if !(0..=59).contains(&seconds_value) {
        return Err(invalid_offset(raw));
    }

    let total_seconds = sign
        * (i64::from(hours) * SECONDS_PER_HOUR
            + i64::from(minutes) * SECONDS_PER_MINUTE
            + i64::from(seconds_value));

    let total_seconds = i32::try_from(total_seconds).map_err(|_| invalid_offset(raw))?;

    UtcOffset::from_whole_seconds(total_seconds).map_err(|err| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("now offset '{raw}' is invalid: {err}"),
        )
    })
}

fn parse_component(component: &str, original: &str) -> Result<i32, Error> {
    component
        .trim()
        .parse::<i32>()
        .map_err(|_| invalid_offset(original))
}

fn invalid_offset(raw: &str) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!("now offset '{raw}' is invalid: expected '+HH:MM' or 'Z'"),
    )
}

fn add_seconds_component(
    mut total: Duration,
    amount: Option<i64>,
    multiplier: i64,
    label: &str,
) -> Result<Duration, Error> {
    if let Some(value) = amount {
        let seconds = value
            .checked_mul(multiplier)
            .ok_or_else(|| overflow_error(label))?;
        let component = Duration::seconds(seconds);
        total = total
            .checked_add(component)
            .ok_or_else(|| overflow_error(label))?;
    }
    Ok(total)
}

fn add_nanoseconds_component(
    mut total: Duration,
    amount: Option<i64>,
    multiplier: i64,
    label: &str,
) -> Result<Duration, Error> {
    if let Some(value) = amount {
        let nanos = value
            .checked_mul(multiplier)
            .ok_or_else(|| overflow_error(label))?;
        let component = Duration::nanoseconds(nanos);
        total = total
            .checked_add(component)
            .ok_or_else(|| overflow_error(label))?;
    }
    Ok(total)
}

fn overflow_error(label: &str) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!("timedelta overflow when adding {label}"),
    )
}

fn format_offset_datetime(datetime: OffsetDateTime) -> String {
    datetime.format(&Iso8601::DEFAULT).map_or_else(
        |_| datetime.to_string(),
        |mut formatted| {
            if let Some(pos) = formatted.find(".000000000")
                && formatted
                    .get(pos + 10..)
                    .and_then(|rest| rest.chars().next())
                    .is_some_and(|next| matches!(next, 'Z' | '+' | '-'))
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
    let mut remainder = absolute - Duration::days(days);

    if days != 0 {
        buffer.push_str(&days.to_string());
        buffer.push('D');
    }

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

    if time_section.is_empty() {
        if buffer.ends_with('P') {
            buffer.push_str("T0S");
        }
    } else {
        buffer.push('T');
        buffer.push_str(&time_section);
    }

    buffer
}

fn format_seconds_with_fraction(seconds: i64, nanos: i32) -> String {
    let seconds = u64::try_from(seconds).expect("seconds must be non-negative");
    if nanos == 0 {
        return format!("{seconds}S");
    }

    let mut fraction = format!("{nanos:09}");
    while fraction.ends_with('0') {
        fraction.pop();
    }

    format!("{seconds}.{fraction}S")
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
