//! Tests for representation-aware generated-Ninja inspection.
//!
//! The cases below cover each transport Netsuke can emit: a plaintext binding,
//! a PowerShell `-EncodedCommand` argument, and a PowerShell response-file
//! payload. They run on every host so the Windows-only transports keep their
//! coverage under the Linux gate.
//!
//! This module is a child of `ninja_semantics`, so it can reach the private
//! [`PowerShellPayload`] decoder and exercise it directly.

use super::{
    GeneratedNinja, PowerShellPayload, RecipeNeedle, RecipeTransport, ResponseFileContent,
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
    let document = GeneratedNinja::new(&generated);
    ensure!(
        document.recipe_contains(RecipeNeedle::new(SENTINEL))?,
        "plaintext recipe text should match"
    );
    ensure!(
        !document.recipe_contains(RecipeNeedle::new(ABSENT_SENTINEL))?,
        "an absent needle must not match plaintext recipe text"
    );
    ensure!(
        document.detected_recipe_transports() == vec![RecipeTransport::Plaintext],
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
    let document = GeneratedNinja::new(&generated);
    ensure!(
        document.recipe_contains(RecipeNeedle::new(SENTINEL))?,
        "the decoded PowerShell recipe should match"
    );
    ensure!(
        document.detected_recipe_transports() == vec![RecipeTransport::PowerShellEncodedCommand],
        "an encoded command binding should report the encoded-command transport"
    );
    Ok(())
}

/// Verify that a PowerShell response-file recipe matches after decoding.
#[test]
fn power_shell_response_file_recipe_matches_after_decoding() -> Result<()> {
    let script = format!("$ErrorActionPreference = 'Stop'\necho {SENTINEL}\n");
    let encoded = encode_power_shell(&script);
    let binding = format!("$$netsukePayload = '{encoded}'; $$netsukeScript = 'bootstrap'");
    let generated = format!("  rspfile = $out.netsuke-large.ps1\n  rspfile_content = {binding}\n");
    ensure!(
        !generated.contains(SENTINEL),
        "the sentinel must not be visible in the generated binding"
    );
    let document = GeneratedNinja::new(&generated);
    ensure!(
        document.recipe_contains(RecipeNeedle::new(SENTINEL))?,
        "the decoded response-file recipe should match"
    );
    ensure!(
        document.detected_recipe_transports() == vec![RecipeTransport::PowerShellResponseFile],
        "a response-file binding should report the response-file transport"
    );
    ensure!(
        ResponseFileContent::new(&binding)
            .decode_power_shell_payload()
            .context("the extracted binding text should decode as a PowerShell payload")?
            == script,
        "the response-file binding should decode to the rendered script"
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
        !GeneratedNinja::new(&generated).recipe_contains(RecipeNeedle::new(needle))?,
        "encoded payload text must not match as plaintext"
    );
    Ok(())
}

/// Verify that plaintext either side of a removed payload cannot be spliced.
#[test]
fn text_around_a_removed_payload_cannot_form_a_match() -> Result<()> {
    let encoded = encode_power_shell("Write-Output done");
    let generated = format!("  command = LEFT-EncodedCommand {encoded}-RIGHT\n");
    ensure!(
        !generated.contains("LEFT-RIGHT"),
        "the spliced needle must not already appear in the generated text"
    );
    ensure!(
        !GeneratedNinja::new(&generated).recipe_contains(RecipeNeedle::new("LEFT-RIGHT"))?,
        "text either side of a removed payload must not be joined into a match"
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
    let error = GeneratedNinja::new(&generated)
        .recipe_contains(RecipeNeedle::new(SENTINEL))
        .err()
        .context("a malformed PowerShell payload must be rejected")?;
    let message = format!("{error:#}");
    ensure!(
        !message.contains(payload),
        "the error message must not echo the encoded payload"
    );
    Ok(())
}

/// Verify that malformed response-file payloads are rejected without echoing them.
#[rstest]
#[case::invalid_padding("QQ==QQ==")]
#[case::odd_length_utf16("QQ==")]
fn malformed_response_file_payloads_are_rejected_without_echoing_them(
    #[case] payload: &str,
) -> Result<()> {
    let binding = format!("$$netsukePayload = '{payload}'; $$netsukeScript = 'bootstrap'");
    let error = ResponseFileContent::new(&binding)
        .decode_power_shell_payload()
        .err()
        .context("a malformed response-file payload must be rejected")?;
    let message = format!("{error:#}");
    ensure!(
        !message.contains(payload),
        "the error message must not echo the encoded payload"
    );
    Ok(())
}

/// Verify that the payload decoder rejects data that cannot be UTF-16LE text.
#[test]
fn power_shell_payload_decoder_rejects_undecodable_payloads() -> Result<()> {
    let odd_byte_count = STANDARD.encode([0x41u8]);
    ensure!(
        PowerShellPayload::new(&odd_byte_count).decode().is_err(),
        "an odd byte count cannot be UTF-16"
    );
    ensure!(
        PowerShellPayload::new("").decode().is_err(),
        "an empty payload cannot be decoded"
    );
    Ok(())
}
