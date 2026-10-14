//! Transport decoders, manifest builders, and prefixes for the composition
//! cases.
//!
//! Split out of the parent to keep it under `AGENTS.md`'s 400-line cap. The
//! decoders are the interesting half: `decode_bash_transport` undoes the Windows
//! argument quoting so its inner script can be run for real, and
//! `decode_power_shell_transport` undoes the base64/UTF-16LE envelope.

use anyhow::{Context, Result, ensure};
use base64::Engine as _;

/// The PowerShell prefix `ninja_gen` puts before the base64 payload.
pub(super) const POWER_SHELL_PREFIX: &str = "powershell.exe -NoLogo -NoProfile -NonInteractive \
                                            -ExecutionPolicy Bypass -EncodedCommand ";

/// The `bash.exe -e -c ` prefix the Bash transport wraps its script in.
pub(super) const BASH_PREFIX: &str = "bash.exe -e -c ";

/// Prefix every line of `text` with `by` spaces.
///
/// Built with `push_str` in a `fold` rather than `map(..).collect()`: the
/// workspace denies `clippy::format_collect`, which flags exactly that shape.
pub(super) fn indent(text: &str, by: usize) -> String {
    let pad = " ".repeat(by);
    text.lines().fold(String::new(), |mut indented, line| {
        indented.push_str(&pad);
        indented.push_str(line);
        indented.push('\n');
        indented
    })
}

/// The whole manifest source for one composition case.
///
/// `recipe` is the target's complete `command:` YAML, written at column zero
/// and indented here. The value reaches the template through a literal block
/// scalar, so no YAML escaping stands between the fixture and the filter, and
/// through a *variable* rather than a spliced literal, so no Jinja escaping
/// does either.
pub(super) fn manifest_with(recipe: &str, value: &str) -> String {
    format!(
        "netsuke_version: \"1.0.0\"\nvars:\n  seam_value: |-\n{value}targets:\n  - name: out\n{recipe}    description: composition\n",
        value = indent(value, 4),
        recipe = indent(recipe, 4),
    )
}

/// The manifest for a scalar recipe whose command is `command`.
pub(super) fn scalar_manifest(command: &str, value: &str) -> String {
    manifest_with(&format!("command: |\n  {command}\n"), value)
}

/// The manifest for a command-list recipe whose single entry is `entry`.
///
/// The entry is a *quoted* YAML scalar: a bare `true` would parse as a YAML
/// boolean and fail `StringOrList` deserialization, and a block scalar would
/// leave the list as one string. The double quotes keep the JSON-escaped `\n`
/// the recipe needs.
pub(super) fn list_manifest(entry: &str, value: &str) -> String {
    manifest_with(
        &format!("command:\n  - \"{}\"\n", entry.replace('\\', "\\\\")),
        value,
    )
}

/// Recover the inner script of the `Bash` transport's `bash.exe -e -c "…"`.
///
/// This is a *decoder*, not a restatement of the encoder: it undoes the
/// Windows argument quoting so the result is the exact text `bash.exe` would
/// receive as its `-c` argument. Running that text under the host's own POSIX
/// shell is what makes the Bash arm an execution test rather than a shape
/// assertion.
pub(super) fn decode_bash_transport(binding: &str) -> Result<String> {
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

/// Decode the PowerShell transport's base64 payload back to its script.
///
/// The renderer prepends a fixed bootstrap; the payload after the prefix is
/// UTF-16LE base64. Decoding it is how the PowerShell arm checks that the
/// *filter's* output reached the interpreter, rather than only that the
/// renderer produced a well-formed command.
pub(super) fn decode_power_shell_transport(binding: &str) -> Result<String> {
    let payload = binding
        .strip_prefix(POWER_SHELL_PREFIX)
        .with_context(|| format!("PowerShell transport should start with the prefix: {binding}"))?;
    let decoded_bytes = base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .context("PowerShell payload should be valid base64")?;
    // `as_chunks` rather than `chunks_exact(2)`, and the `%` test replaced by
    // `is_multiple_of`: the workspace denies `chunks_exact_to_as_chunks`,
    // `integer_division_remainder_used`, `integer_division`,
    // `little_endian_bytes` and `indexing_slicing`.
    let (pairs, remainder) = decoded_bytes.as_chunks::<2>();
    ensure!(
        remainder.is_empty(),
        "UTF-16LE payload should have an even byte count, got {}",
        decoded_bytes.len()
    );
    let code_units = pairs
        .iter()
        .map(|[low, high]| u16::from(*low) | (u16::from(*high) << 8))
        .collect::<Vec<_>>();
    String::from_utf16(&code_units).context("PowerShell payload should be valid UTF-16")
}
