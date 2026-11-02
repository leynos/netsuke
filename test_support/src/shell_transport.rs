//! Decode the shell transports that `RecipeShell` wraps recipe text in.
//!
//! The `Posix` transport puts recipe text on the command line unchanged, but
//! the Windows `Bash` transport hides it inside `bash.exe -e -c "…"` with
//! Windows argument quoting. Decoding that argument recovers the script
//! `bash.exe` would run, so a Unix test can execute it under the host shell.

use anyhow::{Context, Result};

/// The `bash.exe -e -c ` prefix the Bash transport wraps its script in.
const BASH_PREFIX: &str = "bash.exe -e -c ";

/// Recover the inner script of the Bash transport's `bash.exe -e -c "…"`.
///
/// Undoes the Windows argument quoting so the result is the exact text
/// `bash.exe` would receive as its `-c` argument.
///
/// # Errors
///
/// Returns an error when `binding` is not a Bash transport or its argument is
/// not a single quoted word.
pub fn decode_bash_transport(binding: &str) -> Result<String> {
    let argument = binding
        .strip_prefix(BASH_PREFIX)
        .with_context(|| format!("Bash transport should start with {BASH_PREFIX:?}: {binding}"))?;
    let inner = argument
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .with_context(|| {
            format!("Bash transport argument should be one quoted word: {argument}")
        })?;

    // Windows argument quoting: a backslash before a `"` is an escape, and the
    // run of backslashes before the closing quote is halved.
    //
    // The halving is written `>> 1` and the parity test `is_multiple_of`,
    // because the workspace denies `clippy::integer_division` and
    // `clippy::integer_division_remainder_used`: a literal `/` or `%` here
    // would fail the build. Both spell the arithmetic the sentence above names.
    let mut decoded = String::with_capacity(inner.len());
    let mut backslashes = 0usize;
    for character in inner.chars() {
        if character == '\\' {
            backslashes += 1;
            continue;
        }
        if character == '"' && !backslashes.is_multiple_of(2) {
            decoded.push_str(&"\\".repeat(backslashes >> 1));
            decoded.push('"');
        } else {
            decoded.push_str(&"\\".repeat(backslashes));
            decoded.push(character);
        }
        backslashes = 0;
    }
    decoded.push_str(&"\\".repeat(backslashes >> 1));
    Ok(decoded)
}
