//! Possible-value metadata for the CLI policy flags.
//!
//! The domain policy definitions in [`super::config`] stay decoupled from
//! Clap. This module projects their spelling and descriptions into Clap help
//! metadata for the CLI adapter.

use clap::builder::PossibleValue;

use super::config::{
    ACCESSIBILITY_POLICY_DEFINITIONS, COLOUR_POLICY_DEFINITIONS, EMOJI_POLICY_DEFINITIONS,
    PROGRESS_POLICY_DEFINITIONS, PolicyDefinition,
};

/// Convert Clap-independent policy definitions into help metadata.
fn possible_values<T>(definitions: &[PolicyDefinition<T>]) -> Vec<PossibleValue> {
    definitions
        .iter()
        .map(|definition| PossibleValue::new(definition.spelling).help(definition.description))
        .collect()
}

/// Possible values advertised in help for the `--color` flag.
pub fn colour_policy_possible_values() -> Vec<PossibleValue> {
    possible_values(&COLOUR_POLICY_DEFINITIONS)
}

/// Possible values advertised in help for the `--emoji` flag.
pub fn emoji_policy_possible_values() -> Vec<PossibleValue> {
    possible_values(&EMOJI_POLICY_DEFINITIONS)
}

/// Possible values advertised in help for the `--progress` flag.
pub fn progress_policy_possible_values() -> Vec<PossibleValue> {
    possible_values(&PROGRESS_POLICY_DEFINITIONS)
}

/// Possible values advertised in help for the `--accessibility` flag.
pub fn accessibility_policy_possible_values() -> Vec<PossibleValue> {
    possible_values(&ACCESSIBILITY_POLICY_DEFINITIONS)
}
