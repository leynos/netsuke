//! Figment provider for explicitly supplied Netsuke environment values.

use std::ffi::{OsStr, OsString};

use ortho_config::figment::value::{Dict, Map, Value};
use ortho_config::figment::{Error, Metadata, Profile, Provider};

use super::merge::ENV_PREFIX;

/// Fixed rejection text for a key that claims the Netsuke prefix but is not
/// UTF-8. The raw key is never included: it is untrusted, unbounded, and may
/// carry secrets.
const NON_UTF8_KEY: &str = "non-UTF-8 environment key";
/// Fixed rejection text for a non-UTF-8 value. The raw value is never
/// included, for the same reasons.
const NON_UTF8_VALUE: &str = "non-UTF-8 environment value";

/// Environment layer backed by an owned, injected snapshot.
pub(super) struct EnvironmentLayer {
    entries: Vec<(OsString, OsString)>,
}

impl EnvironmentLayer {
    pub(super) const fn new(entries: Vec<(OsString, OsString)>) -> Self {
        Self { entries }
    }
}

impl Provider for EnvironmentLayer {
    fn metadata(&self) -> Metadata {
        Metadata::named("injected environment variables")
    }

    fn data(&self) -> Result<Map<Profile, Dict>, Error> {
        let mut values = Dict::new();
        for (key, value) in &self.entries {
            let entry = parse_entry(key, value).map_err(|error| *error)?;
            if let Some((components, parsed)) = entry {
                insert_nested(&mut values, &components, parsed).map_err(Error::from)?;
            }
        }
        let mut profiles = Map::new();
        profiles.insert(Profile::Default, values);
        Ok(profiles)
    }
}

/// Interpret one environment entry, separating reading from writing.
///
/// Returns the normalized key path and parsed value for an entry this layer
/// owns, `Ok(None)` for one it does not, and an error for malformed
/// configuration. Keeping interpretation here leaves [`EnvironmentLayer::data`]
/// with nothing to do but insert what this yields.
///
/// The error is boxed because `figment::Error` is 208 bytes, which trips
/// `clippy::result_large_err` for anything but a trait method. `data` itself
/// escapes the lint only by virtue of implementing [`Provider`].
fn parse_entry(key: &OsStr, value: &OsStr) -> Result<Option<(Vec<String>, Value)>, Box<Error>> {
    let key_text = match decode_key(key) {
        KeyDecode::Decoded(text) => text,
        KeyDecode::Unrelated => return Ok(None),
        KeyDecode::Invalid => return Err(Box::new(reject(NON_UTF8_KEY, "non_utf8_key"))),
    };
    let Some(stripped) = strip_prefix_uncased(key_text.trim(), ENV_PREFIX) else {
        return Ok(None);
    };
    let components: Vec<String> = stripped
        .split("__")
        .map(str::trim)
        .filter(|component| !component.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    if components.is_empty() {
        return Ok(None);
    }
    let Some(value_text) = value.to_str() else {
        return Err(Box::new(reject(NON_UTF8_VALUE, "non_utf8_value")));
    };
    let parsed = match value_text.parse::<Value>() {
        Ok(parsed) => parsed,
        Err(never) => match never {},
    };
    Ok(Some((components, parsed)))
}

/// Outcome of decoding an environment key.
///
/// This is not a `Result`: `figment::Error` is large enough that returning it
/// from a small helper trips `clippy::result_large_err`, and the caller is the
/// natural place to build the error anyway.
enum KeyDecode<'a> {
    /// Valid UTF-8; the ordinary prefix filter decides whether it is ours.
    Decoded(&'a str),
    /// Not UTF-8 and not addressed to this layer, so it is skipped.
    Unrelated,
    /// Claims the Netsuke prefix but is not UTF-8, so it must be rejected.
    Invalid,
}

/// Decode `key` strictly, distinguishing configuration from unrelated noise.
///
/// The snapshot is the whole process environment, so unrelated variables may
/// legitimately hold non-UTF-8 data and must not fail the load. An entry
/// claiming the Netsuke prefix is configuration this layer owns, so a non-UTF-8
/// key there is an error rather than something to coerce.
fn decode_key(key: &OsStr) -> KeyDecode<'_> {
    match key.to_str() {
        Some(text) => KeyDecode::Decoded(text),
        None if claims_config_prefix(key) => KeyDecode::Invalid,
        None => KeyDecode::Unrelated,
    }
}

/// Build the rejection error, recording only the bounded `failure_kind`.
///
/// Neither the offending key nor its value reaches the log or the error: both
/// are untrusted and may carry secrets.
fn reject(message: &'static str, failure_kind: &'static str) -> Error {
    tracing::warn!(failure_kind, "rejected non-UTF-8 injected configuration");
    Error::from(message.to_owned())
}

/// Report whether `key` claims the Netsuke configuration prefix.
///
/// The prefix is ASCII, so it can be recognised from the encoded bytes without
/// first committing to a UTF-8 decode. This is only consulted for keys that
/// failed a strict decode; valid UTF-8 keys take the ordinary path unchanged.
fn claims_config_prefix(key: &OsStr) -> bool {
    key.as_encoded_bytes()
        .trim_ascii_start()
        .get(..ENV_PREFIX.len())
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(ENV_PREFIX.as_bytes()))
}

fn strip_prefix_uncased<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    value
        .get(..prefix.len())
        .filter(|candidate| candidate.eq_ignore_ascii_case(prefix))
        .and_then(|_| value.get(prefix.len()..))
}

fn insert_nested(target: &mut Dict, components: &[String], value: Value) -> Result<(), String> {
    let Some((head, tail)) = components.split_first() else {
        return Ok(());
    };
    if tail.is_empty() {
        if let Some(existing) = target.get(head) {
            let conflict = if matches!(existing, Value::Dict(..)) {
                "a nested configuration key"
            } else {
                "an existing scalar configuration key"
            };
            return Err(format!(
                "environment key `{}` conflicts with {conflict}",
                components.join("__")
            ));
        }
        target.insert(head.clone(), value);
        return Ok(());
    }
    let entry = target
        .entry(head.clone())
        .or_insert_with(|| Value::from(Dict::new()));
    let Value::Dict(_, nested) = entry else {
        return Err(format!(
            "environment key `{}` conflicts with a scalar configuration key",
            components.join("__")
        ));
    };
    insert_nested(nested, tail, value)
}

#[cfg(test)]
#[path = "environment_tests.rs"]
mod tests;
