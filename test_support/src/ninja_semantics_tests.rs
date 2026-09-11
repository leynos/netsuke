//! Tests for representation-aware generated-Ninja inspection.
//!
//! The cases below cover each transport Netsuke can emit: a plaintext binding,
//! a PowerShell `-EncodedCommand` argument, and a PowerShell response-file
//! payload. They run on every host so the Windows-only transports keep their
//! coverage under the Linux gate.

use super::{
    RecipeTransport, decode_power_shell_script, detected_recipe_transports,
    generated_ninja_contains,
};
use anyhow::{Context, Result, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rstest::rstest;

/// Distinctive sentinel standing in for a recipe value rendered from `env()`.
const SENTINEL: &str = "CI-SECRET-7f3a9c2b5e1d4f60";

/// Sentinel that no generated text contains.
const ABSENT_SENTINEL: &str = "CI-SECRET-absent-from-every-transport";

/// Encode `script` the way PowerShell's `-EncodedCommand` argument carries it.
fn encode_power_shell(script: &str) -> String {
    #[expect(
        clippy::little_endian_bytes,
        reason = "PowerShell -EncodedCommand requires UTF-16LE input"
    )]
    let bytes = script
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<u8>>();
    STANDARD.encode(bytes)
}

/// Verify that plaintext Ninja bindings match directly.
///
/// This is the representation every non-Windows host produces natively, and
/// also what a Windows Bash-compatibility build emits.
#[test]
fn plaintext_ninja_content_matches() -> Result<()> {
    let generated = format!(
        "rule 9a1f\n  command = /bin/sh -e -c \"printf %b 'echo {SENTINEL}' | /bin/sh -e\"\n\nbuild out: 9a1f\n"
    );
    ensure!(
        generated_ninja_contains(&generated, SENTINEL)?,
        "plaintext recipe text should match"
    );
    ensure!(
        !generated_ninja_contains(&generated, ABSENT_SENTINEL)?,
        "an absent needle must not match plaintext recipe text"
    );
    ensure!(
        detected_recipe_transports(&generated) == vec![RecipeTransport::Plaintext],
        "a payload-free file should report the plaintext transport"
    );
    Ok(())
}

/// Verify that an encoded PowerShell recipe matches after decoding.
#[test]
fn encoded_power_shell_recipe_matches_after_decoding() -> Result<()> {
    let script = format!("$ErrorActionPreference = 'Stop'\necho {SENTINEL}\n");
    let encoded = encode_power_shell(&script);
    let generated = format!("  command = powershell.exe -NoProfile -EncodedCommand {encoded}\n");
    ensure!(
        !generated.contains(SENTINEL),
        "the sentinel must not be visible in the generated binding"
    );
    ensure!(
        generated_ninja_contains(&generated, SENTINEL)?,
        "the decoded PowerShell recipe should match"
    );
    ensure!(
        detected_recipe_transports(&generated) == vec![RecipeTransport::PowerShellEncodedCommand],
        "an encoded command binding should report the encoded-command transport"
    );
    Ok(())
}

/// Verify that a PowerShell response-file recipe matches after decoding.
#[test]
fn power_shell_response_file_recipe_matches_after_decoding() -> Result<()> {
    let script = format!("$ErrorActionPreference = 'Stop'\necho {SENTINEL}\n");
    let encoded = encode_power_shell(&script);
    let generated = format!(
        "  rspfile = $out.netsuke-large.ps1\n  rspfile_content = $$netsukePayload = '{encoded}'; $$netsukeScript = 'bootstrap'\n"
    );
    ensure!(
        !generated.contains(SENTINEL),
        "the sentinel must not be visible in the generated binding"
    );
    ensure!(
        generated_ninja_contains(&generated, SENTINEL)?,
        "the decoded response-file recipe should match"
    );
    ensure!(
        detected_recipe_transports(&generated) == vec![RecipeTransport::PowerShellResponseFile],
        "a response-file binding should report the response-file transport"
    );
    Ok(())
}

/// Verify that encoded payload text never matches as plaintext.
#[test]
fn encoded_payload_text_does_not_match_as_plaintext() -> Result<()> {
    let script = "Write-Output done";
    let encoded = encode_power_shell(script);
    let needle = encoded
        .get(4..12)
        .context("the encoded payload should be long enough to sample")?;
    ensure!(
        !script.contains(needle),
        "the sampled needle must not appear in the decoded script"
    );
    let generated = format!("  command = powershell.exe -NoProfile -EncodedCommand {encoded}\n");
    ensure!(
        generated.contains(needle),
        "the sampled needle should appear in the encoded binding text"
    );
    ensure!(
        !generated_ninja_contains(&generated, needle)?,
        "encoded payload text must not match as plaintext"
    );
    Ok(())
}

/// Verify that malformed payloads are rejected without echoing their text.
#[rstest]
#[case::invalid_padding("QQ==QQ==")]
#[case::odd_length_utf16("QQ==")]
fn malformed_power_shell_payloads_are_rejected_without_echoing_them(
    #[case] payload: &str,
) -> Result<()> {
    let generated = format!("  command = powershell.exe -NoProfile -EncodedCommand {payload}\n");
    let error = generated_ninja_contains(&generated, SENTINEL)
        .err()
        .context("a malformed PowerShell payload must be rejected")?;
    let message = format!("{error:#}");
    ensure!(
        !message.contains(payload),
        "the error message must not echo the encoded payload"
    );
    Ok(())
}

/// Verify that odd-length UTF-16 payloads are rejected by the decoder itself.
#[test]
fn decode_power_shell_script_rejects_odd_length_payloads() -> Result<()> {
    let encoded = STANDARD.encode([0x41u8]);
    ensure!(
        decode_power_shell_script(&encoded).is_err(),
        "an odd byte count cannot be UTF-16"
    );
    ensure!(
        decode_power_shell_script("").is_err(),
        "an empty payload cannot be decoded"
    );
    Ok(())
}
