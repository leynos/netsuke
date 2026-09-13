//! Marker scanning for Base64 PowerShell payloads in generated Ninja text.
//!
//! The parent module documents what each transport means to a test; this module
//! locates the payload spans that make those documented answers possible. Split
//! from `ninja_semantics.rs` to keep that module within the Whitaker
//! `module_max_lines` cap; it is included from there via `#[path]` so the
//! scanner stays a child module of `ninja_semantics`.

use std::ops::Range;

use anyhow::{Context, Result};

use super::{GeneratedNinja, PowerShellPayload, RecipeTransport};

/// Prefix that introduces a Base64 UTF-16LE PowerShell script argument.
const POWER_SHELL_ENCODED_COMMAND_PREFIX: &str = "-EncodedCommand ";

/// Ninja binding text that introduces a response-file Base64 payload.
///
/// The leading `$` is deliberately omitted so this marker also matches the
/// `$$` escape Ninja requires in a generated `rspfile_content` binding.
const POWER_SHELL_RESPONSE_FILE_PAYLOAD_PREFIX: &str = "netsukePayload = '";

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
    fn collect(&self, generated: GeneratedNinja<'_>, payloads: &mut Vec<EncodedPayload>) {
        let document = generated.0;
        let mut search_from = 0usize;
        while let Some(offset) = document
            .get(search_from..)
            .and_then(|remaining| remaining.find(self.text))
        {
            let start = search_from + offset + self.text.len();
            let encoded = document.get(start..).unwrap_or_default();
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
pub(super) struct EncodedPayload {
    /// Byte range of the Base64 payload within the generated text.
    span: Range<usize>,
    /// Transport that carries the payload.
    pub(super) transport: RecipeTransport,
}

/// Locate every encoded PowerShell payload in generated Ninja text.
///
/// Payloads are ordered by position so callers can split the text around them.
pub(super) fn locate_encoded_payloads(generated: GeneratedNinja<'_>) -> Vec<EncodedPayload> {
    let mut payloads = Vec::new();
    for marker in &PAYLOAD_MARKERS {
        marker.collect(generated, &mut payloads);
    }
    payloads.sort_by_key(|payload| payload.span.start);
    payloads
}

/// Borrow the Base64 payload of one located marker.
pub(super) fn payload_text<'a>(
    generated: GeneratedNinja<'a>,
    payload: &EncodedPayload,
) -> Result<PowerShellPayload<'a>> {
    generated
        .0
        .get(payload.span.clone())
        .map(PowerShellPayload::new)
        .context("PowerShell payload must lie on UTF-8 boundaries")
}

/// Split generated text into the regions that are not encoded payloads.
///
/// Each region is returned separately rather than concatenated, so text either
/// side of a removed payload cannot be spliced into a false match.
pub(super) fn plaintext_segments<'a>(
    generated: GeneratedNinja<'a>,
    payloads: &[EncodedPayload],
) -> Vec<&'a str> {
    let mut segments = Vec::new();
    let mut cursor = 0usize;
    for payload in payloads {
        if payload.span.start < cursor {
            continue;
        }
        if let Some(segment) = generated.0.get(cursor..payload.span.start) {
            segments.push(segment);
        }
        cursor = payload.span.end;
    }
    if let Some(segment) = generated.0.get(cursor..) {
        segments.push(segment);
    }
    segments
}

/// Count the leading characters of `payload_text` that belong to Base64.
///
/// Every Base64 character is one byte, so the character count is also the byte
/// length of the payload.
fn base64_run_length(payload_text: &str) -> usize {
    let mut length = 0usize;
    for character in payload_text.chars() {
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
