//! Figment provider for explicitly supplied Netsuke environment values.

use std::ffi::{OsStr, OsString};

use ortho_config::figment::value::{Dict, Map, Value};
use ortho_config::figment::{Error, Metadata, Profile, Provider};

use super::constants::{ENV_PREFIX, SHELLS_KEY};

/// Fixed rejection text for a key that claims the Netsuke prefix but is not
/// UTF-8. The raw key is never included: it is untrusted, unbounded, and may
/// carry secrets.
const NON_UTF8_KEY: &str = "non-UTF-8 environment key";
/// Fixed rejection text for a non-UTF-8 value. The raw value is never
/// included, for the same reasons.
const NON_UTF8_VALUE: &str = "non-UTF-8 environment value";
/// Fixed rejection text for the file-only `shells` key.
const SHELLS_IN_ENVIRONMENT: &str =
    "the `shells` key may only be declared in a configuration file, not in the environment";

/// Configuration key selecting JSON diagnostic output.
pub(crate) const JSON_KEY: &str = "json";

/// Complete single-component paths that map to `CliConfig` Boolean fields.
///
/// This list mirrors `verbose`, `json`, `fetch_default_deny`,
/// `trust_project_fetch_policy`, and `no_input` in `src/cli/config/mod.rs`.
/// Nested aliases are deliberately excluded: only the exact configuration key
/// accepts a Boolean environment spelling.
pub(crate) const BOOLEAN_PATHS: [&str; 5] = [
    "verbose",
    JSON_KEY,
    "fetch_default_deny",
    "trust_project_fetch_policy",
    "no_input",
];

/// Parse the strict Boolean spellings accepted from the environment.
///
/// Exactly `true`, `false`, `1`, and `0` are accepted, with no surrounding
/// whitespace and no case folding. Sharing this parser with the early
/// diagnostic-mode resolver keeps the finite accepted set identical before and
/// after the full merge. Configuration files keep strict Boolean typing; this
/// helper applies only to environment values.
pub(crate) fn parse_environment_bool(value: &str) -> Option<bool> {
    match value {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

/// Report whether `components` names a single Boolean configuration path.
fn is_boolean_path(components: &[String]) -> bool {
    match components {
        [component] => BOOLEAN_PATHS.contains(&component.as_str()),
        _ => false,
    }
}

/// Parse one environment value according to its configuration path.
///
/// Boolean paths accept exactly the four strict spellings and otherwise keep
/// the raw text, so case or whitespace variants fail deserialization instead of
/// being coerced. Every other path keeps Figment's ordinary scalar parsing,
/// which recognizes booleans, numbers, arrays, and strings. Figment trims
/// surrounding whitespace from an unquoted scalar, so an environment policy
/// such as `NETSUKE_COLOR=" always "` is accepted even though the same spaces
/// in a quoted configuration-file value are not.
fn parse_value(components: &[String], value_text: &str) -> Value {
    if is_boolean_path(components) {
        return parse_environment_bool(value_text)
            .map_or_else(|| Value::from(value_text.to_owned()), Value::from);
    }
    match value_text.parse::<Value>() {
        Ok(parsed) => parsed,
        Err(never) => match never {},
    }
}

/// Environment layer backed by an owned, injected snapshot.
pub(super) struct EnvironmentLayer {
    /// Owned snapshot of the environment entries this layer presents.
    entries: Vec<(OsString, OsString)>,
}

impl EnvironmentLayer {
    /// Construct a layer from an owned snapshot of environment entries.
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
    let Some(components) = normalized_components(key_text) else {
        return Ok(None);
    };
    // `shells` declares executable policy, so it is a file-only key. Reject it
    // on the normalized leading component, which covers case and whitespace
    // variation as well as nested `NETSUKE_SHELLS__...` spellings.
    if components
        .first()
        .is_some_and(|component| component == SHELLS_KEY)
    {
        return Err(Box::new(reject_disallowed(
            SHELLS_IN_ENVIRONMENT,
            "shells_in_environment",
        )));
    }
    let Some(value_text) = value.to_str() else {
        return Err(Box::new(reject(NON_UTF8_VALUE, "non_utf8_value")));
    };
    let parsed = parse_value(&components, value_text);
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

/// Build the rejection error for an environment key that may not be injected.
///
/// Like [`reject`], this records only the bounded `failure_kind` and never the
/// offending key or value.
fn reject_disallowed(message: &'static str, failure_kind: &'static str) -> Error {
    tracing::warn!(failure_kind, "rejected disallowed injected configuration");
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

/// Normalize one environment key into its configuration path components.
///
/// Surrounding whitespace is trimmed, the `NETSUKE_` prefix is removed without
/// regard to ASCII case, and the remainder is split on the double underscore.
/// Each non-empty component is trimmed and ASCII-lowercased, so `netsuke_json`
/// and ` NETSUKE_JSON ` both name the `json` path.
///
/// The early diagnostic-mode resolver shares this function with the merge
/// layer. A key it honours before the merge is therefore exactly a key the
/// merge honours afterwards; reimplementing the rule in either place would let
/// the two disagree about spellings such as `netsuke_json`.
pub(super) fn normalized_components(key_text: &str) -> Option<Vec<String>> {
    let stripped = strip_prefix_uncased(key_text.trim(), ENV_PREFIX)?;
    let components: Vec<String> = stripped
        .split("__")
        .map(str::trim)
        .filter(|component| !component.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    (!components.is_empty()).then_some(components)
}

/// Return `value` with `prefix` removed when it matches ignoring ASCII case.
fn strip_prefix_uncased<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    value
        .get(..prefix.len())
        .filter(|candidate| candidate.eq_ignore_ascii_case(prefix))
        .and_then(|_| value.get(prefix.len()..))
}

/// Insert `value` into `target` at the nested path `components`, creating
/// intermediate dicts as needed.
///
/// # Errors
///
/// Returns an error when the leaf component conflicts with an existing key,
/// whether that key currently holds a scalar value or a dictionary.
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
mod tests;
