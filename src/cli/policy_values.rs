//! Possible-value metadata for the CLI policy flags.
//!
//! The domain policy enums in [`super::config`] stay decoupled from Clap:
//! they parse through `FromStr` and know nothing about argument presentation.
//! Clap-specific help metadata, including the per-value descriptions derived
//! from the enum variant docs, lives here so the CLI adapter can still
//! advertise accepted values in `--help` output.

use clap::builder::PossibleValue;

/// Possible values advertised in help for the `--color` flag, mirroring the
/// descriptions carried by the `ColourPolicy` variants.
pub fn colour_policy_possible_values() -> Vec<PossibleValue> {
    vec![
        PossibleValue::new("auto").help("Follow the host environment"),
        PossibleValue::new("always").help("Force colour output on when available"),
        PossibleValue::new("never").help("Force colour output off"),
    ]
}

/// Possible values advertised in help for the `--emoji` flag, mirroring the
/// descriptions carried by the `EmojiPolicy` variants.
pub fn emoji_policy_possible_values() -> Vec<PossibleValue> {
    vec![
        PossibleValue::new("auto").help("Follow the host environment and accessibility mode"),
        PossibleValue::new("always").help("Force emoji glyphs on"),
        PossibleValue::new("never").help("Disable emoji glyphs"),
    ]
}

/// Possible values advertised in help for the `--progress` flag, mirroring the
/// descriptions carried by the `ProgressPolicy` variants.
pub fn progress_policy_possible_values() -> Vec<PossibleValue> {
    vec![
        PossibleValue::new("auto").help("Follow Netsuke's default progress behaviour"),
        PossibleValue::new("always").help("Force progress rendering on"),
        PossibleValue::new("never").help("Disable progress rendering"),
    ]
}

/// Possible values advertised in help for the `--accessibility` flag,
/// mirroring the descriptions carried by the `AccessibilityPolicy` variants.
pub fn accessibility_policy_possible_values() -> Vec<PossibleValue> {
    vec![
        PossibleValue::new("auto").help("Follow terminal and environment detection"),
        PossibleValue::new("on").help("Force accessible output on"),
        PossibleValue::new("off").help("Force accessible output off"),
    ]
}
