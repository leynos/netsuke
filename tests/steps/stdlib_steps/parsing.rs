//! Parsing helpers for stdlib Cucumber steps, covering timestamp and offset
//! formats used in output assertions and server host extraction.
use anyhow::{Context, Result, anyhow, bail};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Iso8601};

pub(crate) fn server_host(url: &str) -> Option<&str> {
    extract_host_from_url(url)
}

pub(crate) fn extract_host_from_url(url: &str) -> Option<&str> {
    const HTTP_LEN: usize = 7;
    const HTTPS_LEN: usize = 8;

    if let Some(prefix) = url.get(..HTTP_LEN)
        && prefix.eq_ignore_ascii_case("http://")
    {
        let (_, rest) = url.split_at(HTTP_LEN);
        return rest.split('/').next();
    }

    if let Some(prefix) = url.get(..HTTPS_LEN)
        && prefix.eq_ignore_ascii_case("https://")
    {
        let (_, rest) = url.split_at(HTTPS_LEN);
        return rest.split('/').next();
    }

    None
}

pub(crate) fn parse_iso_timestamp(raw: &str) -> Result<OffsetDateTime> {
    OffsetDateTime::parse(raw, &Iso8601::DEFAULT)
        .with_context(|| format!("parse ISO8601 timestamp from '{raw}'"))
}

/// Extract the sign multiplier and slice away the sign character from an
/// expected UTC offset string.
fn parse_sign(raw: &str) -> Result<(i8, &str)> {
    let mut chars = raw.chars();
    let first = chars
        .next()
        .ok_or_else(|| anyhow!("unsupported offset format: {raw}"))?;
    let rest = chars.as_str();
    match first {
        '+' => Ok((1, rest)),
        '-' => Ok((-1, rest)),
        _ => bail!("unsupported offset format: {raw}"),
    }
}

/// Parse an optional minutes or seconds component, defaulting to zero when the
/// component is not supplied.
fn parse_optional_component(part: Option<&str>, component_name: &str, raw: &str) -> Result<i8> {
    part.map_or(Ok(0), |value| {
        value
            .parse()
            .with_context(|| format!("parse {component_name} component from '{raw}'"))
    })
}

pub(crate) fn parse_expected_offset(raw: &str) -> Result<UtcOffset> {
    let raw = raw.trim();

    if raw.eq_ignore_ascii_case("z") {
        return Ok(UtcOffset::UTC);
    }

    let (sign, rest) = parse_sign(raw)?;

    let mut parts = rest.split(':');
    let hours_part = parts
        .next()
        .ok_or_else(|| anyhow!("offset missing hour component: {raw}"))?
        .trim();
    let hours: i8 = hours_part
        .parse()
        .with_context(|| format!("parse hour component from '{raw}'"))?;
    let minutes: i8 = parse_optional_component(parts.next(), "minute", raw)?;
    let seconds: i8 = parse_optional_component(parts.next(), "second", raw)?;

    if parts.next().is_some() {
        bail!("offset contains unexpected component: {raw}");
    }

    UtcOffset::from_hms(sign * hours, sign * minutes, sign * seconds)
        .with_context(|| format!("offset components out of range in '{raw}'"))
}
