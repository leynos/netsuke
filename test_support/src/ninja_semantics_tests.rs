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

#[path = "ninja_semantics_property_tests.rs"]
mod property_tests;

/// Distinctive sentinel standing in for a recipe value rendered from `env()`.
const SENTINEL: &str = "CI-SECRET-7f3a9c2b5e1d4f60";

/// Sentinel that no generated text contains.
const ABSENT_SENTINEL: &str = "CI-SECRET-absent-from-every-transport";

/// Sentinel carried by the encoded-command payload in mixed-transport text.
const SECOND_SENTINEL: &str = "CI-SECRET-2nd-4b7c1e";

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

/// Verify that a document mixing transports reports each one once, in order.
///
/// The response-file binding appears both before and after the encoded command
/// so the transport list must merge the duplicate rather than repeat it.
#[test]
fn mixed_payloads_report_each_transport_once_in_document_order() -> Result<()> {
    let response_script = format!("$ErrorActionPreference = 'Stop'\necho {SENTINEL}\n");
    let command_script = format!("$ErrorActionPreference = 'Stop'\necho {SECOND_SENTINEL}\n");
    let binding = format!(
        "$$netsukePayload = '{}'; $$netsukeScript = 'bootstrap'",
        encode_power_shell(&response_script)
    );
    let generated = [
        format!("  rspfile_content = {binding}"),
        format!(
            "  command = powershell.exe -NoProfile -EncodedCommand {}",
            encode_power_shell(&command_script)
        ),
        format!("  rspfile_content = {binding}"),
    ]
    .join("\n");
    ensure!(
        !generated.contains(SENTINEL) && !generated.contains(SECOND_SENTINEL),
        "neither sentinel must be visible in the generated bindings"
    );
    let document = GeneratedNinja::new(&generated);
    ensure!(
        document.detected_recipe_transports()
            == vec![
                RecipeTransport::PowerShellResponseFile,
                RecipeTransport::PowerShellEncodedCommand,
            ],
        "mixed payloads should report each transport once, in document order"
    );
    ensure!(
        document.recipe_contains(RecipeNeedle::new(SENTINEL))?,
        "the response-file payload should decode to the script it carries"
    );
    ensure!(
        document.recipe_contains(RecipeNeedle::new(SECOND_SENTINEL))?,
        "the encoded-command payload should decode to the script it carries"
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
    let generated =
        format!("  command = powershell.exe -NoProfile -EncodedCommand {encoded}-RIGHT\n");
    ensure!(
        !generated.contains("-EncodedCommand -RIGHT"),
        "the spliced needle must not already appear in the generated text"
    );
    ensure!(
        !GeneratedNinja::new(&generated)
            .recipe_contains(RecipeNeedle::new("-EncodedCommand -RIGHT"))?,
        "text either side of a removed payload must not be joined into a match"
    );
    Ok(())
}

/// Verify that recipe text mentioning the encoded-command marker stays plaintext.
///
/// Only a `command` binding that invokes the PowerShell executable carries an
/// encoded payload. A recipe that merely prints the marker text keeps its own
/// meaning, so a test searching for it must still find it as plaintext.
#[test]
fn plaintext_command_mentioning_the_marker_is_not_an_encoded_payload() -> Result<()> {
    assert_plaintext_marker_recipe("echo -EncodedCommand QQ==", "encoded-command marker")
}

/// Verify that recipe text mentioning the response-file marker stays plaintext.
#[test]
fn plaintext_command_mentioning_the_response_file_marker_is_not_a_payload() -> Result<()> {
    assert_plaintext_marker_recipe("echo netsukePayload = 'QQ=='", "response-file marker")
}

/// Assert a marker-bearing plaintext recipe remains semantically searchable.
fn assert_plaintext_marker_recipe(recipe: &str, marker_description: &str) -> Result<()> {
    let generated = format!("  command = {recipe}\n");
    let document = GeneratedNinja::new(&generated);
    ensure!(
        document.detected_recipe_transports() == vec![RecipeTransport::Plaintext],
        "the {marker_description} must remain plaintext and semantically searchable"
    );
    ensure!(
        document.recipe_contains(RecipeNeedle::new(recipe))?,
        "the {marker_description} must remain plaintext and semantically searchable"
    );
    Ok(())
}

/// Verify that a bare payload marker is not accepted as response-file content.
#[test]
fn response_file_content_requires_the_binding_value_escaping() -> Result<()> {
    let error = ResponseFileContent::new("netsukePayload = 'QQ=='")
        .decode_power_shell_payload()
        .err()
        .context("a value without the response-file binding escape must be rejected")?;
    ensure!(
        format!("{error:#}").contains("must carry a PowerShell Base64 payload"),
        "the error should report the missing response-file payload, got {error:#}"
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
