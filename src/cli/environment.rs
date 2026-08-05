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
            let key_text = match decode_key(key) {
                KeyDecode::Decoded(text) => text,
                KeyDecode::Unrelated => continue,
                KeyDecode::Invalid => return Err(reject(NON_UTF8_KEY, "non_utf8_key")),
            };
            let Some(stripped) = strip_prefix_uncased(key_text.trim(), ENV_PREFIX) else {
                continue;
            };
            let components: Vec<String> = stripped
                .split("__")
                .map(str::trim)
                .filter(|component| !component.is_empty())
                .map(str::to_ascii_lowercase)
                .collect();
            if components.is_empty() {
                continue;
            }
            let Some(value_text) = value.to_str() else {
                return Err(reject(NON_UTF8_VALUE, "non_utf8_value"));
            };
            let parsed = match value_text.parse::<Value>() {
                Ok(parsed) => parsed,
                Err(never) => match never {},
            };
            insert_nested(&mut values, &components, parsed).map_err(Error::from)?;
        }
        let mut profiles = Map::new();
        profiles.insert(Profile::Default, values);
        Ok(profiles)
    }
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
mod tests {
    //! Unit tests for nested environment-layer construction and conflicts.

    use super::*;
    use proptest::prelude::*;

    fn layer(entries: &[(&str, &str)]) -> EnvironmentLayer {
        EnvironmentLayer::new(
            entries
                .iter()
                .map(|(key, value)| (OsString::from(key), OsString::from(value)))
                .collect(),
        )
    }

    #[test]
    fn provider_filters_prefixes_and_builds_nested_values() {
        let data = layer(&[
            ("IGNORED", "value"),
            ("netsuke_cmds__build__targets", "all"),
        ])
        .data()
        .expect("valid nested environment should produce provider data");
        let defaults = data
            .get(&Profile::Default)
            .expect("provider should emit the default profile");
        let Value::Dict(_, commands) = defaults.get("cmds").expect("cmds dictionary") else {
            panic!("cmds should be a dictionary");
        };
        let Value::Dict(_, build) = commands.get("build").expect("build dictionary") else {
            panic!("build should be a dictionary");
        };
        assert_eq!(build.get("targets").and_then(Value::as_str), Some("all"));
        assert!(!defaults.contains_key("ignored"));
    }

    #[test]
    fn provider_rejects_parent_scalar_before_nested_key() {
        let error = layer(&[
            ("NETSUKE_CMDS", "build"),
            ("NETSUKE_CMDS__BUILD__TARGETS", "all"),
        ])
        .data()
        .expect_err("a scalar parent must conflict with a nested key");
        assert!(error.to_string().contains("scalar configuration key"));
    }

    #[test]
    fn provider_rejects_nested_key_before_parent_scalar() {
        let error = layer(&[
            ("NETSUKE_CMDS__BUILD__TARGETS", "all"),
            ("NETSUKE_CMDS", "build"),
        ])
        .data()
        .expect_err("a nested key must conflict with a scalar parent");
        assert!(error.to_string().contains("nested configuration key"));
    }

    #[test]
    fn provider_rejects_aliases_of_an_existing_scalar_key() {
        let aliases = [
            ("NETSUKE_CMDS__BUILD", "NETSUKE_CMDS____BUILD"),
            ("netsuke_cmds__build", " NETSUKE_CMDS__ BUILD "),
        ];

        for (first, alias) in aliases {
            let error = layer(&[(first, "first"), (alias, "second")])
                .data()
                .expect_err("normalized scalar aliases must conflict");
            assert!(
                error
                    .to_string()
                    .contains("existing scalar configuration key"),
                "normalized alias conflict should identify an existing scalar: {error}"
            );
        }
    }

    #[cfg(unix)]
    mod non_utf8 {
        //! Strict-decode tests for injected configuration.
        //!
        //! Invalid UTF-8 keys and values only exist as `OsString` on Unix,
        //! where `OsStringExt::from_vec` accepts arbitrary bytes.

        use super::super::*;
        use crate::test_tracing_capture::with_test_subscriber;
        use std::os::unix::ffi::OsStringExt;
        use tracing_subscriber::filter::LevelFilter;

        /// Stands in for a secret smuggled through the environment; it must
        /// never reach an error message or a log line.
        const SENTINEL: &[u8] = b"s3cr3t-sentinel";

        fn invalid_bytes(prefix: &[u8]) -> OsString {
            let mut bytes = prefix.to_vec();
            bytes.extend_from_slice(SENTINEL);
            bytes.push(0xFF);
            OsString::from_vec(bytes)
        }

        fn layer(key: OsString, value: OsString) -> EnvironmentLayer {
            EnvironmentLayer::new(vec![(key, value)])
        }

        #[test]
        fn non_utf8_key_is_rejected_with_fixed_text() {
            let error = layer(invalid_bytes(b"NETSUKE_"), OsString::from("value"))
                .data()
                .expect_err("a non-UTF-8 Netsuke key must be rejected");

            assert!(
                error.to_string().contains(NON_UTF8_KEY),
                "expected the fixed key rejection text, got {error}"
            );
            assert!(
                !error.to_string().contains("s3cr3t-sentinel"),
                "the rejected key must not appear in the error: {error}"
            );
        }

        #[test]
        fn non_utf8_value_is_rejected_with_fixed_text() {
            let error = layer(OsString::from("NETSUKE_CMDS__BUILD"), invalid_bytes(b""))
                .data()
                .expect_err("a non-UTF-8 Netsuke value must be rejected");

            assert!(
                error.to_string().contains(NON_UTF8_VALUE),
                "expected the fixed value rejection text, got {error}"
            );
            assert!(
                !error.to_string().contains("s3cr3t-sentinel"),
                "the rejected value must not appear in the error: {error}"
            );
        }

        #[test]
        fn unrelated_non_utf8_entries_are_skipped() {
            let data = layer(invalid_bytes(b"UNRELATED_"), invalid_bytes(b""))
                .data()
                .expect("an unrelated non-UTF-8 entry must not fail the load");

            assert!(
                data.get(&Profile::Default).is_some_and(Dict::is_empty),
                "an unrelated entry should contribute no configuration"
            );
        }

        #[rstest::rstest]
        #[case::key(invalid_bytes(b"NETSUKE_"), OsString::from("value"), "non_utf8_key")]
        #[case::value(
            OsString::from("NETSUKE_CMDS__BUILD"),
            invalid_bytes(b""),
            "non_utf8_value"
        )]
        fn rejection_warns_with_only_a_bounded_failure_kind(
            #[case] key: OsString,
            #[case] value: OsString,
            #[case] failure_kind: &str,
        ) {
            let events = with_test_subscriber(LevelFilter::WARN, |captured| {
                layer(key, value)
                    .data()
                    .expect_err("the invalid entry must be rejected");
                captured.snapshot()
            });

            assert!(
                events.iter().any(|event| event
                    .contains("rejected non-UTF-8 injected configuration")
                    && event.contains(&format!("failure_kind=\"{failure_kind}\""))),
                "expected a bounded rejection warning in {events:?}"
            );
            assert!(
                !events.iter().any(|event| event.contains("s3cr3t-sentinel")),
                "the rejected input must not be logged: {events:?}"
            );
        }
    }

    fn component(name: &str, uppercase: bool, whitespace: &str) -> String {
        let normalized_case = if uppercase {
            name.to_ascii_uppercase()
        } else {
            name.to_ascii_lowercase()
        };
        format!("{whitespace}{normalized_case}{whitespace}")
    }

    proptest! {
        #[test]
        fn normalized_scalar_aliases_never_replace_existing_values(
            prefix_uppercase in any::<bool>(),
            cmds_uppercase in any::<bool>(),
            build_uppercase in any::<bool>(),
            whitespace in "[ \\t]{0,2}",
            separator in prop::sample::select(vec!["__", "____", "______"]),
            reverse_order in any::<bool>(),
        ) {
            let prefix = component("netsuke_", prefix_uppercase, "");
            let cmds = component("cmds", cmds_uppercase, &whitespace);
            let build = component("build", build_uppercase, &whitespace);
            let canonical = String::from("NETSUKE_CMDS__BUILD");
            let alias = format!("{whitespace}{prefix}{cmds}{separator}{build}{whitespace}");
            let entries = if reverse_order {
                [(alias.as_str(), "alias"), (canonical.as_str(), "canonical")]
            } else {
                [(canonical.as_str(), "canonical"), (alias.as_str(), "alias")]
            };

            prop_assert!(layer(&entries).data().is_err());
        }

        #[test]
        fn scalar_and_nested_keys_conflict_in_either_order(reverse_order in any::<bool>()) {
            let entries = if reverse_order {
                [
                    ("NETSUKE_CMDS__BUILD__TARGETS", "all"),
                    ("NETSUKE_CMDS__BUILD", "scalar"),
                ]
            } else {
                [
                    ("NETSUKE_CMDS__BUILD", "scalar"),
                    ("NETSUKE_CMDS__BUILD__TARGETS", "all"),
                ]
            };

            prop_assert!(layer(&entries).data().is_err());
        }
    }
}
