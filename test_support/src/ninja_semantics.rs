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
//! present in plaintext form at all. [`GeneratedNinja::recipe_contains`]
//! searches the plaintext bindings directly and decodes every encoded
//! PowerShell payload before matching, so a call site reads the same on every
//! host.
//!
//! The four text domains involved are not interchangeable: a whole generated
//! document, text to search for in its recipe semantics, Base64 payload text,
//! and Ninja response-file binding text are each wrapped in their own borrowed
//! newtype. Accepting a bare `&str` for any of them would let a caller search
//! an encoded payload as if it were Ninja text, or decode a whole document as
//! if it were a payload.
//!
//! Name the active representation in a failure message with
//! [`GeneratedNinja::detected_recipe_transports`] rather than echoing the
//! generated file, which may contain rendered secret material interpolated
//! through `env()`.

#[path = "ninja_semantics_scanner.rs"]
mod scanner;

use self::scanner::{locate_encoded_payloads, payload_text, plaintext_segments};
use anyhow::{Context, Result, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};

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
    ///
    /// # Examples
    ///
    /// ```
    /// use test_support::ninja_semantics::RecipeTransport;
    /// assert_eq!(RecipeTransport::Plaintext.description(), "plaintext Ninja binding");
    /// ```
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Plaintext => "plaintext Ninja binding",
            Self::PowerShellEncodedCommand => "PowerShell -EncodedCommand argument",
            Self::PowerShellResponseFile => "PowerShell response file",
        }
    }
}

/// A complete generated Ninja document.
///
/// This is the only type that knows how a document hides its recipe text, so it
/// owns payload location, exclusion of encoded spans from plaintext matching,
/// and payload decoding. Distinct from `netsuke::ninja_gen::GeneratedNinja`,
/// which is the production generator's output type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedNinja<'a>(
    /// The whole generated manifest, exactly as written to disk.
    &'a str,
);

impl<'a> GeneratedNinja<'a> {
    /// View `text` as a generated Ninja document.
    ///
    /// # Examples
    ///
    /// ```
    /// use test_support::ninja_semantics::GeneratedNinja;
    /// let document = GeneratedNinja::new("build out: echo\n");
    /// assert_eq!(document.detected_recipe_transports().len(), 1);
    /// ```
    #[must_use]
    pub const fn new(text: &'a str) -> Self {
        Self(text)
    }

    /// Report how this document carries recipe text.
    ///
    /// Each distinct transport is returned once; a document with no encoded
    /// PowerShell payload reports [`RecipeTransport::Plaintext`], which is also
    /// what a Windows Bash-compatibility build produces.
    ///
    /// # Examples
    ///
    /// ```
    /// use test_support::ninja_semantics::{GeneratedNinja, RecipeTransport};
    /// let document = GeneratedNinja::new("build out: echo\n");
    /// assert_eq!(document.detected_recipe_transports(), [RecipeTransport::Plaintext]);
    /// ```
    #[must_use]
    pub fn detected_recipe_transports(self) -> Vec<RecipeTransport> {
        let mut transports = Vec::new();
        for payload in locate_encoded_payloads(self) {
            if !transports.contains(&payload.transport) {
                transports.push(payload.transport);
            }
        }
        if transports.is_empty() {
            transports.push(RecipeTransport::Plaintext);
        }
        transports
    }

    /// Return whether this document's recipe semantics contain `needle`.
    ///
    /// Plaintext bindings are searched directly. Every PowerShell payload is
    /// decoded first and the encoded text never matches, so a Windows recipe is
    /// compared as the script PowerShell will actually run.
    ///
    /// # Examples
    ///
    /// ```
    /// use test_support::ninja_semantics::{GeneratedNinja, RecipeNeedle};
    /// let document = GeneratedNinja::new("  command = echo hi\n");
    /// assert!(matches!(document.recipe_contains(RecipeNeedle::new("echo hi")), Ok(true)));
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when a PowerShell payload is empty or is not valid Base64
    /// UTF-16LE. The payload text is never included in the error.
    pub fn recipe_contains(self, needle: RecipeNeedle<'_>) -> Result<bool> {
        let payloads = locate_encoded_payloads(self);
        let mut decoded = Vec::with_capacity(payloads.len());
        for payload in &payloads {
            decoded.push(payload_text(self, payload)?.decode()?);
        }
        if plaintext_segments(self, &payloads)
            .iter()
            .any(|segment| segment.contains(needle.as_str()))
        {
            return Ok(true);
        }
        Ok(decoded
            .iter()
            .any(|script| script.contains(needle.as_str())))
    }
}

/// Text to search for in generated Ninja recipe semantics.
///
/// Wrapping the needle keeps it distinct from the whole document searched, so a
/// call site cannot silently swap the two.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecipeNeedle<'a>(
    /// The text a recipe must carry to match.
    &'a str,
);

impl<'a> RecipeNeedle<'a> {
    /// Search generated Ninja recipe semantics for `text`.
    ///
    /// # Examples
    ///
    /// ```
    /// use test_support::ninja_semantics::{GeneratedNinja, RecipeNeedle};
    /// let document = GeneratedNinja::new("  command = echo hi\n");
    /// let needle = RecipeNeedle::new("echo hi");
    /// assert!(matches!(document.recipe_contains(needle), Ok(true)));
    /// ```
    #[must_use]
    pub const fn new(text: &'a str) -> Self {
        Self(text)
    }

    /// Borrow the search text.
    const fn as_str(self) -> &'a str {
        self.0
    }
}

/// A Base64 UTF-16LE PowerShell script embedded in generated Ninja text.
///
/// Private on purpose: callers decode through the document or response file
/// that carries the payload, so the Base64 and UTF-16LE rules stay in one
/// place.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PowerShellPayload<'a>(
    /// Base64 text exactly as it appears in the generated document.
    &'a str,
);

impl<'a> PowerShellPayload<'a> {
    /// Borrow `text` as a Base64 PowerShell payload.
    const fn new(text: &'a str) -> Self {
        Self(text)
    }

    /// Decode the UTF-16LE PowerShell script this payload carries.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload is empty or is not Base64 UTF-16LE.
    /// The payload text is never included in the error.
    fn decode(self) -> Result<String> {
        ensure!(!self.0.is_empty(), "PowerShell payload must not be empty");
        let bytes = STANDARD
            .decode(self.0)
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
}

/// The `rspfile_content` binding text of a Ninja response file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResponseFileContent<'a>(
    /// Binding text as generated, including Ninja's `$$` escaping.
    &'a str,
);

impl<'a> ResponseFileContent<'a> {
    /// View `text` as a Ninja `rspfile_content` binding.
    ///
    /// # Examples
    ///
    /// ```
    /// use test_support::ninja_semantics::ResponseFileContent;
    /// let binding = "$$netsukePayload = 'QQA='";
    /// let script = ResponseFileContent::new(binding).decode_power_shell_payload();
    /// assert!(matches!(script, Ok(text) if text == "A"));
    /// ```
    #[must_use]
    pub const fn new(text: &'a str) -> Self {
        Self(text)
    }

    /// Decode the Base64 UTF-16LE recipe this binding embeds.
    ///
    /// # Examples
    ///
    /// ```
    /// use test_support::ninja_semantics::ResponseFileContent;
    /// let binding = "$$netsukePayload = 'QQA='";
    /// let script = ResponseFileContent::new(binding).decode_power_shell_payload();
    /// assert!(matches!(script, Ok(text) if text == "A"));
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the binding carries no PowerShell response-file
    /// payload, or when that payload is not valid Base64 UTF-16LE. The payload
    /// text is never included in the error.
    pub fn decode_power_shell_payload(self) -> Result<String> {
        let document = GeneratedNinja::new(self.0);
        let payload = locate_encoded_payloads(document)
            .into_iter()
            .find(|payload| payload.transport == RecipeTransport::PowerShellResponseFile)
            .context("response file content must carry a PowerShell Base64 payload")?;
        payload_text(document, &payload)?.decode()
    }
}

#[cfg(test)]
#[path = "ninja_semantics_tests.rs"]
mod tests;
