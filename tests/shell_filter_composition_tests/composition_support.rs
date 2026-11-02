//! Transport decoders, manifest builders, and prefixes for the composition
//! cases.
//!
//! Split out of the parent to keep it under `AGENTS.md`'s 400-line cap. The
//! decoders are the interesting half: `decode_bash_transport` undoes the Windows
//! argument quoting so its inner script can be run for real, and
//! `decode_power_shell_transport` undoes the base64/UTF-16LE envelope. The Bash
//! decoder is shared with the Ninja dollar-escaping suite, so it lives in
//! `test_support` and is re-exported here to keep the parent's import list.

use anyhow::{Context, Result, ensure};
use base64::Engine as _;

pub(super) use test_support::shell_transport::decode_bash_transport;

/// The absolute interpreter the composition cases bind into every plan.
///
/// These cases select an explicit shell, so the renderer requires a binding
/// and quotes it as one Windows argument before the fixed shell arguments.
pub(super) const BOUND_EXECUTABLE: &str = "\"/fixtures/netsuke/interpreter\"";

/// The fixed arguments the PowerShell transport puts after its executable.
const POWER_SHELL_ARGUMENTS: &str =
    " -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -EncodedCommand ";

/// Build the prefix that introduces a transport's payload.
fn bound_prefix(arguments: &str) -> String {
    format!("{BOUND_EXECUTABLE}{arguments}")
}


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


/// Decode the PowerShell transport's base64 payload back to its script.
///
/// The renderer prepends a fixed bootstrap; the payload after the prefix is
/// UTF-16LE base64. Decoding it is how the PowerShell arm checks that the
/// *filter's* output reached the interpreter, rather than only that the
/// renderer produced a well-formed command.
pub(super) fn decode_power_shell_transport(binding: &str) -> Result<String> {
    let prefix = bound_prefix(POWER_SHELL_ARGUMENTS);
    let payload = binding
        .strip_prefix(&prefix)
        .with_context(|| format!("PowerShell transport should start with {prefix:?}: {binding}"))?;
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
// weave: run 'weave explain tests/shell_filter_composition_tests/composition_support.rs' for per-hunk detail, 'weave check' to verify your resolution
