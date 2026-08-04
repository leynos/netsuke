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
            insert_nested(&mut values, &components, parsed);
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

fn insert_nested(target: &mut Dict, components: &[String], value: Value) {
    let Some((head, tail)) = components.split_first() else {
        return;
    };
    if tail.is_empty() {
        target.insert(head.clone(), value);
        return;
    }
    let entry = target
        .entry(head.clone())
        .or_insert_with(|| Value::from(Dict::new()));
    if !matches!(entry, Value::Dict(..)) {
        *entry = Value::from(Dict::new());
    }
    if let Value::Dict(_, nested) = entry {
        insert_nested(nested, tail, value);
    }
}
