use anyhow::{Context, Result, anyhow, bail};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Iso8601};

pub(crate) fn server_host(url: &str) -> Option<&str> {
    extract_host_from_url(url)
}

pub(crate) fn extract_host_from_url(url: &str) -> Option<&str> {
    let addr = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))?;
    addr.split('/').next()
}

pub(crate) fn parse_iso_timestamp(raw: &str) -> Result<OffsetDateTime> {
    OffsetDateTime::parse(raw, &Iso8601::DEFAULT)
        .with_context(|| format!("parse ISO8601 timestamp from '{raw}'"))
}

pub(crate) fn parse_expected_offset(raw: &str) -> Result<UtcOffset> {
    if raw.eq_ignore_ascii_case("z") {
        return Ok(UtcOffset::UTC);
    }

    let mut chars = raw.chars();
    let first = chars
        .next()
        .ok_or_else(|| anyhow!("unsupported offset format: {raw}"))?;
    let rest = chars.as_str();
    let (sign, rest) = match first {
        '+' => (1, rest),
        '-' => (-1, rest),
        _ => bail!("unsupported offset format: {raw}"),
    };

    let mut parts = rest.split(':');
    let hours: i8 = parts
        .next()
        .ok_or_else(|| anyhow!("offset missing hour component: {raw}"))?
        .parse()
        .with_context(|| format!("parse hour component from '{raw}'"))?;
    let minutes: i8 = parts.next().map_or(Ok(0), |value| {
        value
            .parse()
            .with_context(|| format!("parse minute component from '{raw}'"))
    })?;
    let seconds: i8 = parts.next().map_or(Ok(0), |value| {
        value
            .parse()
            .with_context(|| format!("parse second component from '{raw}'"))
    })?;

    UtcOffset::from_hms(sign * hours, sign * minutes, sign * seconds)
        .with_context(|| format!("offset components out of range in '{raw}'"))
}
