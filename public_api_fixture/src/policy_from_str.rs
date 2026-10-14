//! Type-check Clap-independent policy parsing.
//!
//! This crate does not depend on Clap, so naming a Clap item here would not
//! compile: each policy type must parse through the standard-library
//! `FromStr` boundary alone.

use netsuke::cli::{AccessibilityPolicy, ColourPolicy, EmojiPolicy, ProgressPolicy};
use std::str::FromStr;

/// Every public policy type, parsed from one spelling each.
pub struct ParsedPolicies {
    /// The parsed colour policy.
    pub colour: ColourPolicy,
    /// The parsed emoji policy.
    pub emoji: EmojiPolicy,
    /// The parsed progress policy.
    pub progress: ProgressPolicy,
    /// The parsed accessibility policy.
    pub accessibility: AccessibilityPolicy,
}

/// Parse a policy through its public standard-library trait boundary.
///
/// # Errors
///
/// Returns the policy type's rejection message for an unknown spelling.
pub fn parse_policy<T>(raw: &str) -> Result<T, String>
where
    T: FromStr<Err = String>,
{
    raw.parse()
}

/// Parse every public policy type without importing Clap.
///
/// # Errors
///
/// Returns the first policy type's rejection message.
pub fn parse_every_policy() -> Result<ParsedPolicies, String> {
    Ok(ParsedPolicies {
        colour: parse_policy("AUTO")?,
        emoji: parse_policy("Always")?,
        progress: parse_policy("nEvEr")?,
        accessibility: parse_policy("oN")?,
    })
}
