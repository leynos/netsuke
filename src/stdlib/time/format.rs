//! ISO-8601 rendering for the standard-library time values.
//!
//! Kept separate from [`super`]'s clock and arithmetic helpers so the module
//! stays within the repository's 400-line cap. The renders round-trip through
//! `MiniJinja` as lightweight objects so predicates can downcast them later
//! without reparsing strings.

use std::fmt;
use std::sync::Arc;

use minijinja::value::{Object, ObjectRepr, Value};
use time::{Duration, OffsetDateTime, UtcOffset, format_description::well_known::Iso8601};

use super::{SECONDS_PER_HOUR_I32, SECONDS_PER_MINUTE_I32};

/// Return whether a timezone marker follows the index at `pos`.
fn has_timezone_after(formatted: &str, pos: usize) -> bool {
    formatted
        .get(pos + 10..)
        .and_then(|rest| rest.chars().next())
        .is_some_and(|next| matches!(next, 'Z' | '+' | '-'))
}

/// Render an offset datetime as ISO-8601, dropping the zero fractional part.
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

/// Render a UTC offset as a sign and colon-separated hours, minutes, seconds.
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

/// Render a duration as its ISO-8601 `P..DThh:mm:ss` representation.
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

/// Render the time portion of an ISO-8601 duration from `remainder`.
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

/// Append the time section to the buffer, defaulting to `T0S` when empty.
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

/// Render whole seconds with a trimmed fractional part, or plain seconds.
fn format_seconds_with_fraction(seconds: i64, nanos: i32) -> String {
    // Callers pass the remainder of an absolute duration, so `seconds` is
    // non-negative; `unsigned_abs` is the total conversion for that domain.
    debug_assert!(seconds >= 0, "seconds must be non-negative (got {seconds})");
    let seconds_u64 = seconds.unsigned_abs();
    if nanos == 0 {
        return format!("{seconds_u64}S");
    }

    let mut fraction = format!("{nanos:09}");
    while fraction.ends_with('0') {
        fraction.pop();
    }

    format!("{seconds_u64}.{fraction}S")
}

/// A timestamp the `now` helper returns as an object with string attributes.
#[derive(Clone, Copy)]
pub(super) struct TimestampValue {
    /// The wrapped UTC datetime. Crate-visible so `tests.rs` can read it back.
    pub(crate) datetime: OffsetDateTime,
}

impl TimestampValue {
    /// Wrap a datetime value.
    pub(super) const fn new(datetime: OffsetDateTime) -> Self {
        Self { datetime }
    }

    /// Render the wrapped datetime in ISO-8601 form.
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

/// A duration the `timedelta` helper returns as an object with attributes.
#[derive(Clone, Copy)]
pub(super) struct TimeDeltaValue {
    /// The wrapped duration. Crate-visible so `tests.rs` can read it back.
    pub(crate) duration: Duration,
}

impl TimeDeltaValue {
    /// Wrap a duration value.
    pub(super) const fn new(duration: Duration) -> Self {
        Self { duration }
    }

    /// Render the wrapped duration in ISO-8601 form.
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
