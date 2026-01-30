//! Shared CLI tokenization helpers for BDD steps.
//!
//! This module centralizes shell-style argument splitting to keep BDD
//! scenarios consistent across step definitions.

use std::ffi::OsString;

/// Build an argv-style token list from a CLI argument string.
///
/// Uses shell-like splitting via `shlex` to handle quoted arguments. Falls
/// back to whitespace splitting if `shlex` fails (e.g., unbalanced quotes).
pub fn build_tokens(args: &str) -> Vec<OsString> {
    let mut tokens = vec![OsString::from("netsuke")];
    let trimmed = args.trim();
    if trimmed.is_empty() {
        return tokens;
    }
    match shlex::split(trimmed) {
        Some(split_args) => tokens.extend(split_args.into_iter().map(OsString::from)),
        None => tokens.extend(trimmed.split_whitespace().map(OsString::from)),
    }
    tokens
}
