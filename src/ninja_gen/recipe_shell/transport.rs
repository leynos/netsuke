//! Encode, quote and measure the payloads recipe rendering produces.
//!
//! Two transports live here. A `script` recipe may span many lines, while a
//! Ninja `command` binding must remain on one, so the POSIX and Bash renderers
//! carry the script through `/bin/sh -e -c "printf %b '<operand>' | /bin/sh -e"`.
//! Script text crosses three parsers on that route, and each encoder in the
//! *POSIX script transport* section below targets exactly one of them; see the
//! design document's *POSIX script transport encoding* section.
//!
//! The PowerShell helpers turn an already-selected recipe into the bytes,
//! argument text and command shape a Windows process launch expects. They are
//! split from the renderer so the renderer module stays within the repository's
//! module line cap; the renderer remains the only caller of both groups.

use base64::{Engine as _, engine::general_purpose::STANDARD};

use super::super::NinjaGenError;
use super::super::escape::{NinjaValue, ShellText, escape_ninja_value};

/// Represent one Ninja command binding and its optional response-file payload.
pub(crate) enum RenderedRecipeCommand {
    /// Run the command directly from the Ninja command binding.
    Direct(NinjaValue),
    /// Let Ninja materialize a response file for one build edge.
    ResponseFile {
        /// Fixed command that starts the response file as a PowerShell script.
        command: NinjaValue,
        /// One-line bootstrap script that contains Base64 UTF-16LE recipe text.
        content: NinjaValue,
    },
}

impl RenderedRecipeCommand {
    /// Borrow the command text written into the Ninja rule.
    pub(crate) const fn command(&self) -> &NinjaValue {
        match self {
            Self::Direct(command) | Self::ResponseFile { command, .. } => command,
        }
    }

    /// Borrow the optional response-file content written by Ninja at execution time.
    pub(crate) const fn response_file_content(&self) -> Option<&NinjaValue> {
        match self {
            Self::Direct(_) => None,
            Self::ResponseFile { content, .. } => Some(content),
        }
    }
}

/// Count the UTF-16 code units Windows includes in one command line.
pub(super) fn command_line_characters(prefix: &str, payload: &str) -> usize {
    prefix
        .encode_utf16()
        .count()
        .saturating_add(payload.encode_utf16().count())
}

/// Start the one-line response-file bootstrap before its Base64 payload.
pub(super) const POWER_SHELL_RESPONSE_FILE_SCRIPT_PREFIX: &str = "$netsukePayload = '";

/// Finish the one-line response-file bootstrap after its Base64 payload.
pub(super) const POWER_SHELL_RESPONSE_FILE_SCRIPT_SUFFIX: &str = concat!(
    "'; $netsukeScript = try { [Text.Encoding]::Unicode.GetString([Convert]::FromBase64String($netsukePayload)) } ",
    "catch { throw \"Netsuke could not decode the PowerShell response file: $($_.Exception.Message)\" }; ",
    "try { . ([ScriptBlock]::Create($netsukeScript)) } finally { Remove-Item -LiteralPath $PSCommandPath -Force }; ",
    "if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }"
);

/// Encode PowerShell source as the UTF-16LE Base64 accepted by `-EncodedCommand`.
pub(super) fn encode_power_shell_script(script: &str) -> String {
    let utf16le = script
        .encode_utf16()
        .flat_map(|code_unit| {
            #[expect(
                clippy::little_endian_bytes,
                reason = "PowerShell -EncodedCommand requires UTF-16LE input"
            )]
            let bytes = code_unit.to_le_bytes();
            bytes
        })
        .collect::<Vec<_>>();
    STANDARD.encode(utf16le)
}

/// Build the one-line PowerShell bootstrap stored in Ninja's response file.
///
/// The response file is itself a `-File` script so Windows PowerShell receives
/// its path through normal argument parsing. The payload remains Base64 because
/// Ninja bindings cannot safely carry a multi-line legacy recipe directly.
pub(super) fn power_shell_response_file_content(
    encoded_script: &str,
) -> Result<NinjaValue, NinjaGenError> {
    let bootstrap = format!(
        "{POWER_SHELL_RESPONSE_FILE_SCRIPT_PREFIX}{encoded_script}{POWER_SHELL_RESPONSE_FILE_SCRIPT_SUFFIX}"
    );
    escape_ninja_value(&ShellText::new(bootstrap))
}

/// Quote one argument according to the Windows `CommandLineToArgvW` convention.
pub(super) fn windows_argument(argument: &str) -> String {
    let mut quoted = String::with_capacity(argument.len() + 2);
    quoted.push('"');
    let mut backslashes = 0usize;
    for character in argument.chars() {
        if character == '\\' {
            backslashes += 1;
        } else if character == '"' {
            quoted.push_str(&"\\".repeat(backslashes.saturating_mul(2).saturating_add(1)));
            quoted.push('"');
            backslashes = 0;
        } else {
            quoted.push_str(&"\\".repeat(backslashes));
            quoted.push(character);
            backslashes = 0;
        }
    }
    quoted.push_str(&"\\".repeat(backslashes.saturating_mul(2)));
    quoted.push('"');
    quoted
}

/// Encode one POSIX script as the operand of the wrapper's `printf %b`.
///
/// This is the innermost layer of the POSIX script transport. `printf %b`
/// interprets backslash escapes, so every literal backslash is doubled and
/// every line feed becomes the two characters `\n`. The doubling must happen
/// before any outer layer adds backslashes of its own; otherwise the outer
/// shell consumes the protection and `printf %b` reconstructs an escape from
/// argument data.
fn encode_printf_operand(script: &str) -> String {
    let mut encoded = String::with_capacity(script.len());
    for character in script.chars() {
        match character {
            '\\' => encoded.push_str("\\\\"),
            '\n' => encoded.push_str("\\n"),
            _ => encoded.push(character),
        }
    }
    encoded
}

/// Encode the wrapper's `printf %b` operand as one POSIX single-quoted word.
///
/// This is the middle layer, consumed by the `/bin/sh` that runs
/// `printf %b '<operand>'`. A literal apostrophe closes the word, contributes
/// an escaped apostrophe, and reopens it, which is the standard single-quote
/// escape in POSIX shell.
fn encode_single_quoted_word(word: &str) -> String {
    word.replace('\'', r"'\''")
}

/// Encode one inner word for the wrapper's outer double-quoted string.
///
/// This is the outermost layer, consumed by the shell that receives
/// `/bin/sh -e -c "…"`. Inside double quotes only backslash, dollar, double
/// quote, and backtick are special, so each is prefixed with a backslash. The
/// backslash escape runs last, after the inner layers have added their own, so
/// a literal backslash becomes four backslashes in generated text and reaches
/// the final interpreter as one.
fn encode_double_quoted_word(word: &str) -> String {
    let mut encoded = String::with_capacity(word.len());
    for character in word.chars() {
        if matches!(character, '\\' | '$' | '"' | '`') {
            encoded.push('\\');
        }
        encoded.push(character);
    }
    encoded
}

/// Escape a POSIX script for the wrapper's single-quoted `printf %b` argument.
///
/// The wrapper is `/bin/sh -e -c "printf %b '<escaped>' | /bin/sh -e"`, so the
/// script crosses three parsers before the final interpreter reads it: the
/// outer shell's double quotes, the inner shell's single-quoted `%b` operand,
/// and `printf %b` itself. Each encoder above targets exactly one parser, and
/// they compose from the innermost outwards. Reordering them lets one layer
/// consume another's backslash protection and reconstruct shell syntax from
/// argument data.
pub(in crate::ninja_gen) fn escape_posix_script(script: &str) -> String {
    let operand = encode_printf_operand(script);
    let word = encode_single_quoted_word(&operand);
    encode_double_quoted_word(&word)
}
