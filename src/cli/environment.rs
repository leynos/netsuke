//! Figment provider for explicitly supplied Netsuke environment values.

use std::ffi::OsString;

use ortho_config::figment::value::{Dict, Map, Value};
use ortho_config::figment::{Error, Metadata, Profile, Provider};

use super::merge::ENV_PREFIX;

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
            let key_text = key.to_string_lossy();
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
            let parsed = match value.to_string_lossy().parse::<Value>() {
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
