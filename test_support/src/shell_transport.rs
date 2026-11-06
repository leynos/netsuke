//! Decode the shell transports that `RecipeShell` wraps recipe text in.
//!
//! The `Posix` transport puts recipe text on the command line unchanged, but
//! the Windows `Bash` transport hides it inside `"<executable>" -e -c "…"`
//! with Windows argument quoting. Decoding that argument recovers the script
//! the Bash interpreter would run, so a Unix test can execute it under the
//! host shell.

use anyhow::{Context, Result, ensure};

/// The absolute interpreter path every suite binds into a Bash recipe.
///
/// The path need not exist: the tests decode the rendered command instead of
/// executing it through this binding. It is absolute because the renderer
/// refuses a relative interpreter, which would resolve through `PATH`.
pub const INTERPRETER_PATH: &str = "/fixtures/netsuke/interpreter";

/// The fixed arguments the Bash transport puts after its executable.
const BASH_ARGUMENTS: &str = " -e -c ";

/// Recover the inner script from the transport built for `executable`.
///
/// The renderer always quotes its executable, so the binding is
/// `"<executable>" -e -c "<script>"`. The executable is compared against the
/// expectation so a transport that ignored the binding cannot decode here:
/// silently accepting `bash.exe` would hide exactly the substitution this
/// argument guards against.
///
/// # Errors
///
/// Returns an error when `binding` is not a Bash transport, when it names a
/// different executable, or when its script is not a single quoted word.
pub fn decode_bash_transport(executable: &str, binding: &str) -> Result<String> {
    let (named, argument) = binding
        .strip_prefix('"')
        .and_then(|rest| rest.split_once('"'))
        .with_context(|| format!("Bash transport should name its executable: {binding}"))?;
    ensure!(
        named == executable,
        "Bash transport should run {executable}, not {named}: {binding}"
    );
    let argument = argument.strip_prefix(BASH_ARGUMENTS).with_context(|| {
        format!("Bash transport should pass {BASH_ARGUMENTS:?} after its executable: {binding}")
    })?;
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
