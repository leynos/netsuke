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

/// Ninja binding text that introduces a PowerShell command carrying that argument.
const POWER_SHELL_COMMAND_BINDING_PREFIX: &str = "command = powershell.exe ";

/// Ninja binding text that introduces a response-file Base64 payload.
///
/// The leading `$` is deliberately omitted so this marker also matches the
/// `$$` escape Ninja requires in a generated `rspfile_content` binding.
const POWER_SHELL_RESPONSE_FILE_PAYLOAD_PREFIX: &str = "netsukePayload = '";

/// Ninja binding name that carries the response-file bootstrap.
const RESPONSE_FILE_CONTENT_BINDING_PREFIX: &str = "rspfile_content = ";

/// Binding context a marker must appear in to carry a payload.
///
/// A recipe is written verbatim into a plaintext binding, so recipe text can
/// contain marker text as well as a transport can. Only the bindings the
/// renderer writes introduce a payload; a marker anywhere else is recipe text
/// and stays searchable as plaintext.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PayloadBinding {
    /// A `command` binding that invokes the PowerShell executable.
    PowerShellCommand,
    /// The value of a Ninja `rspfile_content` binding.
    ResponseFileContent,
}

impl PayloadBinding {
    /// Report whether the binding text `prefix` introduces a payload.
    ///
    /// `prefix` is the text between the start of the binding's line and the
    /// marker, with indentation removed. The response-file rule accepts both a
    /// generated `rspfile_content = ` binding and that binding's value alone,
    /// because [`super::ResponseFileContent`] wraps the bare value and the
    /// generated value carries Ninja's `$$` escaping.
    fn accepts(self, prefix: &str) -> bool {
        match self {
            Self::PowerShellCommand => prefix.starts_with(POWER_SHELL_COMMAND_BINDING_PREFIX),
            Self::ResponseFileContent => {
                let value = prefix
                    .strip_prefix(RESPONSE_FILE_CONTENT_BINDING_PREFIX)
                    .unwrap_or(prefix);
                !value.is_empty() && value.bytes().all(|byte| byte == b'$')
            }
        }
    }
}

/// Binding text that introduces a Base64 payload, plus its transport.
struct PayloadMarker {
    /// Text immediately preceding the Base64 payload.
    text: &'static str,
    /// Binding that must introduce the marker for it to carry a payload.
    binding: PayloadBinding,
    /// Transport that carries the payload.
    transport: RecipeTransport,
}

impl PayloadMarker {
    /// Record every payload this marker introduces into `payloads`.
    ///
    /// An occurrence of the marker text outside this marker's binding is recipe
    /// text, so scanning resumes after the marker text rather than recording a
    /// payload for it. Scanning resumes after a recorded payload rather than
    /// after its marker, because Base64 cannot contain the marker text, so no
    /// marker can hide inside a payload.
    fn collect(&self, generated: GeneratedNinja<'_>, payloads: &mut Vec<EncodedPayload>) {
        let document = generated.0;
        let mut search_from = 0usize;
        while let Some(offset) = document
            .get(search_from..)
            .and_then(|remaining| remaining.find(self.text))
        {
            let marker_start = search_from + offset;
            let start = marker_start + self.text.len();
            if !self.binding.accepts(line_prefix(document, marker_start)) {
                search_from = start;
                continue;
            }
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

/// Borrow the text of the binding that holds `offset`, without its indentation.
///
/// The result is the text between the start of `offset`'s line and `offset`
/// itself, so a binding check describes the run of text that introduces a
/// marker rather than the whole line a marker happens to sit on.
fn line_prefix(document: &str, offset: usize) -> &str {
    let line_start = document
        .get(..offset)
        .and_then(|before| before.rfind('\n'))
        .map_or(0, |newline| newline + 1);
    document
        .get(line_start..offset)
        .unwrap_or_default()
        .trim_start()
}

/// Every marker that introduces a Base64 PowerShell payload.
const PAYLOAD_MARKERS: [PayloadMarker; 2] = [
    PayloadMarker {
        text: POWER_SHELL_ENCODED_COMMAND_PREFIX,
        binding: PayloadBinding::PowerShellCommand,
        transport: RecipeTransport::PowerShellEncodedCommand,
    },
    PayloadMarker {
        text: POWER_SHELL_RESPONSE_FILE_PAYLOAD_PREFIX,
        binding: PayloadBinding::ResponseFileContent,
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
