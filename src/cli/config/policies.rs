//! Display policies shared by configuration files, the environment, and the CLI.
//!
//! Each enum resolves spellings through
//! [`super::policy_definitions`], so the command-line `FromStr` parser and
//! configuration deserialization accept exactly the same case-insensitive set.
//! Serde serialization continues to emit the canonical lowercase spelling.

use super::policy_definitions::{
    ACCESSIBILITY_POLICY_DEFINITIONS, COLOUR_POLICY_DEFINITIONS, EMOJI_POLICY_DEFINITIONS,
    PROGRESS_POLICY_DEFINITIONS, PolicyDefinition, definition_for, parse_policy,
};
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

/// Deserialize a policy value by delegating to its case-insensitive `FromStr`.
///
/// Serialization keeps the canonical lowercase spellings, while `FromStr`
/// defines the accepted, case-insensitive input set already used by the
/// command line. Failure text names the accepted spellings so a configuration
/// typo is actionable without consulting the guide.
fn deserialize_policy<'de, D, T>(
    deserializer: D,
    definitions: &[PolicyDefinition<T>],
) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: FromStr<Err = String>,
{
    let raw = String::deserialize(deserializer)?;
    T::from_str(&raw).map_err(|error| {
        let expected = definitions
            .iter()
            .map(|definition| definition.spelling)
            .collect::<Vec<_>>()
            .join(", ");
        serde::de::Error::custom(format!("{error}; expected one of: {expected}"))
    })
}

/// Colour-output policy accepted by layered configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ColourPolicy {
    /// Follow the host environment.
    #[default]
    Auto,
    /// Force colour output on when available.
    Always,
    /// Force colour output off.
    Never,
}

impl fmt::Display for ColourPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        definition_for(*self, &COLOUR_POLICY_DEFINITIONS).map_or(Err(fmt::Error), |definition| {
            f.write_str(definition.spelling)
        })
    }
}

impl FromStr for ColourPolicy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_policy(s, &COLOUR_POLICY_DEFINITIONS)
            .ok_or_else(|| format!("invalid color policy '{s}'"))
    }
}

impl<'de> Deserialize<'de> for ColourPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserialize_policy(deserializer, &COLOUR_POLICY_DEFINITIONS)
    }
}

/// Progress rendering policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ProgressPolicy {
    /// Follow Netsuke's default progress behaviour.
    #[default]
    Auto,
    /// Force progress rendering on.
    Always,
    /// Disable progress rendering.
    Never,
}

impl fmt::Display for ProgressPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        definition_for(*self, &PROGRESS_POLICY_DEFINITIONS).map_or(Err(fmt::Error), |definition| {
            f.write_str(definition.spelling)
        })
    }
}

impl FromStr for ProgressPolicy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_policy(s, &PROGRESS_POLICY_DEFINITIONS)
            .ok_or_else(|| format!("invalid progress policy '{s}'"))
    }
}

impl<'de> Deserialize<'de> for ProgressPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserialize_policy(deserializer, &PROGRESS_POLICY_DEFINITIONS)
    }
}

/// Emoji rendering policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum EmojiPolicy {
    /// Follow the host environment and accessibility mode.
    #[default]
    Auto,
    /// Force emoji glyphs on.
    Always,
    /// Disable emoji glyphs.
    Never,
}

impl fmt::Display for EmojiPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        definition_for(*self, &EMOJI_POLICY_DEFINITIONS).map_or(Err(fmt::Error), |definition| {
            f.write_str(definition.spelling)
        })
    }
}

impl FromStr for EmojiPolicy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_policy(s, &EMOJI_POLICY_DEFINITIONS)
            .ok_or_else(|| format!("invalid emoji policy '{s}'"))
    }
}

impl<'de> Deserialize<'de> for EmojiPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserialize_policy(deserializer, &EMOJI_POLICY_DEFINITIONS)
    }
}

/// Accessible-output policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AccessibilityPolicy {
    /// Follow terminal and environment detection.
    #[default]
    Auto,
    /// Force accessible output on.
    On,
    /// Force accessible output off.
    Off,
}

impl fmt::Display for AccessibilityPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        definition_for(*self, &ACCESSIBILITY_POLICY_DEFINITIONS)
            .map_or(Err(fmt::Error), |definition| {
                f.write_str(definition.spelling)
            })
    }
}

impl FromStr for AccessibilityPolicy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_policy(s, &ACCESSIBILITY_POLICY_DEFINITIONS)
            .ok_or_else(|| format!("invalid accessibility policy '{s}'"))
    }
}

impl<'de> Deserialize<'de> for AccessibilityPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserialize_policy(deserializer, &ACCESSIBILITY_POLICY_DEFINITIONS)
    }
}
