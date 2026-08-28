//! Canonical domain definitions for configuration policy values.
//!
//! This module centralizes policy spellings, variants, and descriptions without
//! importing Clap. The CLI-only [`super::policy_values`] adapter projects these
//! definitions into `PossibleValue` metadata.

use super::{AccessibilityPolicy, ColourPolicy, EmojiPolicy, ProgressPolicy};

/// One accepted policy spelling, its domain variant, and help description.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PolicyDefinition<T> {
    /// Spelling accepted from configuration and CLI arguments.
    pub(crate) spelling: &'static str,
    /// Domain variant selected by the spelling.
    pub(crate) variant: T,
    /// User-visible explanation of the policy value.
    pub(crate) description: &'static str,
}

/// Find the definition that names `variant`.
pub(crate) fn definition_for<T: Copy + Eq>(
    variant: T,
    definitions: &[PolicyDefinition<T>],
) -> Option<PolicyDefinition<T>> {
    definitions
        .iter()
        .copied()
        .find(|definition| definition.variant == variant)
}

/// Parse `raw` according to the accepted, case-insensitive policy spellings.
pub(crate) fn parse_policy<T: Copy>(raw: &str, definitions: &[PolicyDefinition<T>]) -> Option<T> {
    definitions
        .iter()
        .find(|definition| definition.spelling.eq_ignore_ascii_case(raw))
        .map(|definition| definition.variant)
}

/// Canonical colour-policy spellings, variants, and help descriptions.
pub(crate) const COLOUR_POLICY_DEFINITIONS: [PolicyDefinition<ColourPolicy>; 3] = [
    PolicyDefinition {
        spelling: "auto",
        variant: ColourPolicy::Auto,
        description: "Follow the host environment",
    },
    PolicyDefinition {
        spelling: "always",
        variant: ColourPolicy::Always,
        description: "Force colour output on when available",
    },
    PolicyDefinition {
        spelling: "never",
        variant: ColourPolicy::Never,
        description: "Force colour output off",
    },
];

/// Canonical progress-policy spellings, variants, and help descriptions.
pub(crate) const PROGRESS_POLICY_DEFINITIONS: [PolicyDefinition<ProgressPolicy>; 3] = [
    PolicyDefinition {
        spelling: "auto",
        variant: ProgressPolicy::Auto,
        description: "Follow Netsuke's default progress behaviour",
    },
    PolicyDefinition {
        spelling: "always",
        variant: ProgressPolicy::Always,
        description: "Force progress rendering on",
    },
    PolicyDefinition {
        spelling: "never",
        variant: ProgressPolicy::Never,
        description: "Disable progress rendering",
    },
];

/// Canonical emoji-policy spellings, variants, and help descriptions.
pub(crate) const EMOJI_POLICY_DEFINITIONS: [PolicyDefinition<EmojiPolicy>; 3] = [
    PolicyDefinition {
        spelling: "auto",
        variant: EmojiPolicy::Auto,
        description: "Follow the host environment and accessibility mode",
    },
    PolicyDefinition {
        spelling: "always",
        variant: EmojiPolicy::Always,
        description: "Force emoji glyphs on",
    },
    PolicyDefinition {
        spelling: "never",
        variant: EmojiPolicy::Never,
        description: "Disable emoji glyphs",
    },
];

/// Canonical accessibility-policy spellings, variants, and help descriptions.
pub(crate) const ACCESSIBILITY_POLICY_DEFINITIONS: [PolicyDefinition<AccessibilityPolicy>; 3] = [
    PolicyDefinition {
        spelling: "auto",
        variant: AccessibilityPolicy::Auto,
        description: "Follow terminal and environment detection",
    },
    PolicyDefinition {
        spelling: "on",
        variant: AccessibilityPolicy::On,
        description: "Force accessible output on",
    },
    PolicyDefinition {
        spelling: "off",
        variant: AccessibilityPolicy::Off,
        description: "Force accessible output off",
    },
];
