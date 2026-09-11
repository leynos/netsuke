//! Representation-aware inspection of generated Ninja recipe text.
//!
//! Netsuke lowers a completed legacy recipe through one of three transports.
//! The POSIX and Bash transports leave recipe text visible in a plaintext
//! Ninja binding, while the Windows PowerShell transport hides it inside a
//! Base64 UTF-16LE `-EncodedCommand` argument, or inside Ninja's response-file
//! bootstrap for recipes too large for a command line.
//!
//! Tests that assert on recipe semantics therefore cannot scan generated Ninja
//! output as plain text: on Windows the text they are looking for is not
//! present in plaintext form at all. [`generated_ninja_contains`] searches the
//! plaintext bindings directly and decodes every encoded PowerShell payload
//! before matching, so a call site reads the same on every host.
//!
//! Name the active representation in a failure message with
//! [`detected_recipe_transports`] rather than echoing the generated file, which
//! may contain rendered secret material interpolated through `env()`.

use std::ops::Range;

use anyhow::{Context, Result, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};

/// Prefix that introduces a Base64 UTF-16LE PowerShell script argument.
const POWER_SHELL_ENCODED_COMMAND_PREFIX: &str = "-EncodedCommand ";

/// Ninja binding text that introduces a response-file Base64 payload.
///
/// The leading `$` is deliberately omitted so this marker also matches the
/// `$$` escape Ninja requires in a generated `rspfile_content` binding.
const POWER_SHELL_RESPONSE_FILE_PAYLOAD_PREFIX: &str = "netsukePayload = '";

/// One legacy-recipe transport that generated Ninja text can carry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecipeTransport {
    /// Recipe text is visible verbatim in a plaintext Ninja binding.
    Plaintext,
    /// Recipe text is Base64 UTF-16LE inside a PowerShell `-EncodedCommand` argument.
    PowerShellEncodedCommand,
    /// Recipe text is Base64 UTF-16LE inside a Ninja response-file payload.
    PowerShellResponseFile,
}

impl RecipeTransport {
    /// Describe this transport for a test failure message.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Plaintext => "plaintext Ninja binding",
            Self::PowerShellEncodedCommand => "PowerShell -EncodedCommand argument",
            Self::PowerShellResponseFile => "PowerShell response file",
        }
    }
}

/// Binding text that introduces a Base64 payload, plus its transport.
struct PayloadMarker {
    /// Text immediately preceding the Base64 payload.
    text: &'static str,
    /// Transport that carries the payload.
    transport: RecipeTransport,
}

impl PayloadMarker {
    /// Record every payload this marker introduces into `payloads`.
    ///
    /// Scanning resumes after each payload rather than after each marker
    /// because Base64 cannot contain the marker text, so no marker can hide
    /// inside a payload.
    fn collect(&self, generated: &str, payloads: &mut Vec<EncodedPayload>) {
        let mut search_from = 0usize;
        while let Some(offset) = generated
            .get(search_from..)
            .and_then(|remaining| remaining.find(self.text))
        {
            let start = search_from + offset + self.text.len();
            let encoded = generated.get(start..).unwrap_or_default();
            let end = start + base64_run_length(encoded);
            payloads.push(EncodedPayload {
                span: start..end,
                transport: self.transport,
            });
            search_from = end;
        }
    }
}

/// Every marker that introduces a Base64 PowerShell payload.
const PAYLOAD_MARKERS: [PayloadMarker; 2] = [
    PayloadMarker {
        text: POWER_SHELL_ENCODED_COMMAND_PREFIX,
        transport: RecipeTransport::PowerShellEncodedCommand,
    },
    PayloadMarker {
        text: POWER_SHELL_RESPONSE_FILE_PAYLOAD_PREFIX,
        transport: RecipeTransport::PowerShellResponseFile,
    },
];

/// One Base64 PowerShell payload located in generated Ninja text.
struct EncodedPayload {
    /// Byte range of the Base64 payload within the generated text.
    span: Range<usize>,
    /// Transport that carries the payload.
    transport: RecipeTransport,
}

/// Report how generated Ninja text carries recipe text.
///
/// Each distinct transport the file uses is returned once. A file with no
/// encoded PowerShell payload reports [`RecipeTransport::Plaintext`], which is
/// also what a Windows Bash-compatibility build produces.
#[must_use]
pub fn detected_recipe_transports(generated: &str) -> Vec<RecipeTransport> {
    let mut transports = Vec::new();
    for payload in locate_encoded_payloads(generated) {
        if !transports.contains(&payload.transport) {
            transports.push(payload.transport);
        }
    }
    if transports.is_empty() {
        transports.push(RecipeTransport::Plaintext);
    }
    transports
}

/// Return whether generated Ninja recipe semantics contain `needle`.
///
/// Plaintext bindings are searched directly. Every PowerShell payload is
/// decoded first, and the encoded text itself never matches, so a Windows
/// `-EncodedCommand` or response-file recipe is compared as the script
/// PowerShell will actually run.
///
/// # Errors
///
/// Returns an error when a PowerShell payload is empty or is not valid Base64
/// UTF-16LE. The payload text is never included in the error.
pub fn generated_ninja_contains(generated: &str, needle: &str) -> Result<bool> {
    let payloads = locate_encoded_payloads(generated);
    let mut decoded = Vec::with_capacity(payloads.len());
    for payload in &payloads {
        decoded.push(decode_power_shell_script(payload_text(
            generated, payload,
        )?)?);
    }
    if plaintext_segments(generated, &payloads)
        .iter()
        .any(|segment| segment.contains(needle))
    {
        return Ok(true);
    }
    Ok(decoded.iter().any(|script| script.contains(needle)))
}

/// Decode the UTF-16LE PowerShell payload emitted in a Ninja command binding.
///
/// # Errors
///
/// Returns an error when `encoded` is empty or is not Base64 UTF-16LE. The
/// payload text is never included in the error.
pub fn decode_power_shell_script(encoded: &str) -> Result<String> {
    ensure!(!encoded.is_empty(), "PowerShell payload must not be empty");
    let bytes = STANDARD
        .decode(encoded)
        .context("decode PowerShell command payload")?;
    let (pairs, remainder) = bytes.as_chunks::<2>();
    ensure!(
        remainder.is_empty(),
        "PowerShell UTF-16 payload must contain an even number of bytes"
    );
    let units = pairs
        .iter()
        .map(|[low, high]| u16::from(*low) | (u16::from(*high) << 8))
        .collect::<Vec<_>>();
    String::from_utf16(&units).context("decode PowerShell UTF-16 payload")
}

/// Decode the Base64 UTF-16LE recipe embedded in a Ninja response-file binding.
///
/// `response_file_content` is the `rspfile_content` binding text as generated,
/// including Ninja's `$$` escaping.
///
/// # Errors
///
/// Returns an error when the binding carries no PowerShell response-file
/// payload, or when that payload is not valid Base64 UTF-16LE. The payload
/// text is never included in the error.
pub fn decode_response_file_payload(response_file_content: &str) -> Result<String> {
    let payload = locate_encoded_payloads(response_file_content)
        .into_iter()
        .find(|payload| payload.transport == RecipeTransport::PowerShellResponseFile)
        .context("response file content must carry a PowerShell Base64 payload")?;
    let encoded = payload_text(response_file_content, &payload)?;
    decode_power_shell_script(encoded)
}

/// Locate every encoded PowerShell payload in generated Ninja text.
///
/// Payloads are ordered by position so callers can split the text around them.
fn locate_encoded_payloads(generated: &str) -> Vec<EncodedPayload> {
    let mut payloads = Vec::new();
    for marker in &PAYLOAD_MARKERS {
        marker.collect(generated, &mut payloads);
    }
    payloads.sort_by_key(|payload| payload.span.start);
    payloads
}

/// Borrow the Base64 text of one located payload.
fn payload_text<'a>(generated: &'a str, payload: &EncodedPayload) -> Result<&'a str> {
    generated
        .get(payload.span.clone())
        .context("PowerShell payload must lie on UTF-8 boundaries")
}

/// Split generated text into the regions that are not encoded payloads.
///
/// Each region is returned separately rather than concatenated, so text either
/// side of a removed payload cannot be spliced into a false match.
fn plaintext_segments<'a>(generated: &'a str, payloads: &[EncodedPayload]) -> Vec<&'a str> {
    let mut segments = Vec::new();
    let mut cursor = 0usize;
    for payload in payloads {
        if payload.span.start < cursor {
            continue;
        }
        if let Some(segment) = generated.get(cursor..payload.span.start) {
            segments.push(segment);
        }
        cursor = payload.span.end;
    }
    if let Some(segment) = generated.get(cursor..) {
        segments.push(segment);
    }
    segments
}

/// Count the leading characters of `text` that belong to the Base64 alphabet.
///
/// Every Base64 character is one byte, so the character count is also the byte
/// length of the payload.
fn base64_run_length(text: &str) -> usize {
    let mut length = 0usize;
    for character in text.chars() {
        if !is_base64_alphabet(character) {
            break;
        }
        length += 1;
    }
    length
}

/// Test whether `character` belongs to the standard Base64 alphabet.
const fn is_base64_alphabet(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '+' || character == '/' || character == '='
}

#[cfg(test)]
#[path = "ninja_semantics_tests.rs"]
mod tests;
