//! Environment adapters used by manifest BDD steps.

use std::sync::Arc;

use netsuke::manifest;

use crate::bdd::fixtures::TestWorld;

/// Build a [`manifest::EnvReader`] backed by the scenario's forwarded
/// environment so manifest and IR steps read scenario values rather than the
/// host environment.
pub(crate) fn manifest_env_reader(world: &TestWorld) -> manifest::EnvReader {
    let values = world.env_vars_forward.borrow().clone();
    Arc::new(move |key| {
        values
            .get(key)
            .cloned()
            .ok_or(manifest::EnvReadError::NotPresent)?
            .into_string()
            .map_err(|_| manifest::EnvReadError::NotUnicode)
    })
}

fn parse_env_token<I>(world: &TestWorld, chars: &mut std::iter::Peekable<I>) -> String
where
    I: Iterator<Item = char>,
{
    chars.next();
    let mut name = String::new();
    for ch in chars.by_ref() {
        if ch == '}' {
            break;
        }
        name.push(ch);
    }
    world
        .env_vars_forward
        .borrow()
        .get(&name)
        .and_then(|value| value.to_str().map(str::to_owned))
        .unwrap_or_else(|| ["${", &name, "}"].concat())
}

/// Expand `${VAR}` patterns using the scenario's child environment.
pub(super) fn expand_env(world: &TestWorld, raw: &str) -> String {
    let mut output = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '$' && chars.peek() == Some(&'{') {
            output.push_str(&parse_env_token(world, &mut chars));
        } else {
            output.push(character);
        }
    }
    output
}
