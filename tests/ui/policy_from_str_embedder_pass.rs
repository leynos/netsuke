//! Compile-pass fixture for Clap-independent policy parsing.

use netsuke::cli::{AccessibilityPolicy, ColourPolicy, EmojiPolicy, ProgressPolicy};
use std::str::FromStr;

/// Parse a policy through its public standard-library trait boundary.
fn parse_policy<T>(raw: &str) -> T
where
    T: FromStr<Err = String>,
{
    raw.parse().expect("fixture uses only accepted policy spellings")
}

/// Exercise every public policy type without importing Clap.
fn main() {
    let _: ColourPolicy = parse_policy("AUTO");
    let _: EmojiPolicy = parse_policy("Always");
    let _: ProgressPolicy = parse_policy("nEvEr");
    let _: AccessibilityPolicy = parse_policy("oN");
}
