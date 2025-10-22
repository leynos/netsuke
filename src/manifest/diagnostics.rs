//! Translates manifest parsing errors into actionable diagnostics.
use miette::{Diagnostic, NamedSource, SourceSpan};
use serde_saphyr::{Error as YamlError, Location};
use thiserror::Error;

/// YAML source content for a manifest.
///
/// # Examples
/// ```rust
/// use netsuke::manifest::ManifestSource;
/// let source = ManifestSource::from("foo: 1");
/// assert_eq!(source.as_str(), "foo: 1");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ManifestSource(String);

impl ManifestSource {
    /// Construct a new manifest source buffer from any owned string type.
    #[must_use]
    pub fn new(src: impl Into<String>) -> Self {
        Self(src.into())
    }

    /// View the stored source contents as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<&str> for ManifestSource {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ManifestSource {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl AsRef<str> for ManifestSource {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Display for ManifestSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

/// Display name for a manifest source used in diagnostics.
///
/// # Examples
/// ```rust
/// use netsuke::manifest::ManifestName;
/// let name = ManifestName::new("Netsukefile");
/// assert_eq!(name.as_str(), "Netsukefile");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ManifestName(String);

impl ManifestName {
    /// Construct a diagnostic label describing the manifest being processed.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Access the label as a borrowed string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<&str> for ManifestName {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ManifestName {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl AsRef<str> for ManifestName {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Display for ManifestName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

use super::hints::YAML_HINTS;

fn location_to_index(src: &ManifestSource, loc: Location) -> usize {
    byte_index(src, loc)
}

fn byte_index(src: &ManifestSource, loc: Location) -> usize {
    byte_index_components(src.as_ref(), loc.line(), loc.column())
}

/// Reconstruct the byte offset for a `serde_saphyr::Location`.
///
/// `serde_saphyr` exposes only line and column accessors, so we derive the
/// byte index by iterating over the manifest source directly. The logic clamps
/// offsets that exceed the current line and tolerates both Unix (`\n`) and
/// Windows (`\r\n`) newlines.
fn byte_index_components(src: &str, line: u64, column: u64) -> usize {
    let target_line = usize::try_from(line.saturating_sub(1)).unwrap_or(usize::MAX);
    let target_column = usize::try_from(column.saturating_sub(1)).unwrap_or(usize::MAX);
    let mut offset = 0usize;
    for (idx, segment) in src.split_inclusive('\n').enumerate() {
        if idx == target_line {
            let without_newline = segment.strip_suffix('\n').unwrap_or(segment);
            let cleaned_line = without_newline
                .strip_suffix('\r')
                .unwrap_or(without_newline);
            let column_offset = cleaned_line
                .char_indices()
                .nth(target_column)
                .map_or(cleaned_line.len(), |(byte_idx, _)| byte_idx);
            return offset + column_offset;
        }
        offset += segment.len();
    }
    src.len()
}

fn to_span(src: &ManifestSource, loc: Location) -> SourceSpan {
    let at = location_to_index(src, loc);
    let bytes = src.as_ref().as_bytes();
    let is_line_break = |b: u8| b == b'\n' || b == b'\r';
    let (start, end) = match bytes.get(at) {
        Some(&b) if !is_line_break(b) => (at, at + 1),
        _ => {
            let start = if at > 0 && bytes.get(at - 1).is_some_and(|p| !is_line_break(*p)) {
                at - 1
            } else {
                at
            };
            (start, at)
        }
    };
    let len = end.saturating_sub(start);
    #[expect(clippy::useless_conversion, reason = "future-proof span length type")]
    SourceSpan::new(start.into(), len.into())
}

#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
#[diagnostic(code(netsuke::yaml::parse))]
struct YamlDiagnostic {
    #[source_code]
    src: NamedSource<String>,
    #[label("parse error here")]
    span: Option<SourceSpan>,
    #[help]
    help: Option<String>,
    #[source]
    source: YamlError,
    message: String,
}

fn has_tab_indent(src: &ManifestSource, location: Option<Location>) -> bool {
    let Some(actual_loc) = location else {
        return false;
    };
    let line_idx = usize::try_from(actual_loc.line().saturating_sub(1)).unwrap_or(usize::MAX);
    let line = src.as_ref().lines().nth(line_idx).unwrap_or("");
    line.chars()
        .take_while(|c| c.is_whitespace())
        .any(|c| c == '\t')
}

fn hint_for(err_str: &str, src: &ManifestSource, loc: Option<Location>) -> Option<String> {
    if has_tab_indent(src, loc) {
        return Some("Use spaces for indentation; tabs are invalid in YAML.".into());
    }
    let lower = err_str.to_lowercase();
    YAML_HINTS
        .iter()
        .find(|(needle, _)| lower.contains(*needle))
        .map(|(_, hint)| (*hint).into())
}

#[derive(Debug, Error, Diagnostic)]
/// Error raised when manifest parsing fails.
///
/// # Examples
/// ```rust
/// use miette::miette;
/// use netsuke::manifest::ManifestError;
///
/// let err = ManifestError::Parse { source: Box::new(miette!("bad manifest")) };
/// assert_eq!(format!("{err}"), "manifest parse error");
/// ```
pub enum ManifestError {
    /// Manifest parsing failed and produced the supplied diagnostic.
    #[error("manifest parse error")]
    #[diagnostic(code(netsuke::manifest::parse))]
    Parse {
        #[source]
        #[diagnostic_source]
        /// Underlying diagnostic reported by the parser or validator.
        source: Box<dyn Diagnostic + Send + Sync + 'static>,
    },
}

/// Map a `serde_saphyr` YAML parse error into a [`miette`] diagnostic.
///
/// The diagnostic includes the offending span when `serde_saphyr` reports byte
/// offsets, and attempts to attach contextual hints for common mistakes such as
/// tab indentation.
#[must_use]
pub fn map_yaml_error(
    err: YamlError,
    src: &ManifestSource,
    name: &ManifestName,
) -> Box<dyn Diagnostic + Send + Sync + 'static> {
    let loc = err.location();
    let (line, col, span) = loc.map_or((1, 1, None), |l| {
        (l.line(), l.column(), Some(to_span(src, l)))
    });
    let err_str = err.to_string();
    let hint = hint_for(&err_str, src, loc);
    let mut message = format!("YAML parse error at line {line}, column {col}: {err_str}");
    if let Some(ref h) = hint {
        message.push_str("\nhelp: ");
        message.push_str(h);
    }

    Box::new(YamlDiagnostic {
        src: NamedSource::new(name.as_ref(), src.as_ref().to_owned()),
        span,
        help: hint,
        source: err,
        message,
    })
}

#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
#[diagnostic(code(netsuke::manifest::structure))]
struct DataDiagnostic {
    #[source]
    source: serde_json::Error,
    message: String,
}

/// Map a [`serde_json`] structural error into a diagnostic without a source
/// span. `serde_json` does not report byte offsets for data validation failures,
/// so the resulting diagnostic only carries the manifest name and error
/// message.
#[must_use]
pub fn map_data_error(
    err: serde_json::Error,
    name: &ManifestName,
) -> Box<dyn Diagnostic + Send + Sync + 'static> {
    let message = format!(
        "[netsuke::manifest::structure] manifest structure error in {}: {err}",
        name.as_ref()
    );
    Box::new(DataDiagnostic {
        source: err,
        message,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, reason = "tests inspect diagnostic internals")]
    use super::*;
    use std::error::Error as StdError;

    #[test]
    fn map_yaml_error_includes_tab_hint() {
        let src = ManifestSource::from("\tkey: \"unterminated");
        let err = serde_saphyr::from_str::<crate::manifest::ManifestValue>(src.as_ref())
            .expect_err("expected parse error");
        let name = ManifestName::from("test");
        let diag = map_yaml_error(err, &src, &name);
        let msg = diag.to_string();
        assert!(msg.contains("Use spaces for indentation"), "message: {msg}");
    }

    #[test]
    fn map_yaml_error_defaults_location_when_missing() {
        let src = ManifestSource::from("foo: [1");
        let err = serde_saphyr::Error::Eof {
            location: serde_saphyr::Location::UNKNOWN,
        };
        let name = ManifestName::from("test");
        let diag = map_yaml_error(err, &src, &name);
        assert!(diag.to_string().contains("line 1, column 1"));
    }

    #[test]
    fn map_yaml_error_span_skips_carriage_return() {
        let src = ManifestSource::from("targets:\r\n  - name: hi\r\n    command echo\r\n");
        let err = serde_saphyr::from_str::<crate::manifest::ManifestValue>(src.as_ref())
            .expect_err("expected parse error");
        let name = ManifestName::from("test");
        let diag = map_yaml_error(err, &src, &name);
        let yaml_diag = (&*diag as &(dyn StdError + 'static))
            .downcast_ref::<YamlDiagnostic>()
            .expect("expected YAML diagnostic");
        let span = yaml_diag.span.expect("span present");
        let offset = span.offset();
        if let Some(byte) = src.as_ref().as_bytes().get(offset) {
            assert_ne!(*byte, b'\r');
        }
    }

    #[test]
    fn location_to_index_handles_utf8() {
        // café: 'é' is multi-byte
        let src = ManifestSource::from("café: [\n");
        let err = serde_saphyr::from_str::<crate::manifest::ManifestValue>(src.as_ref())
            .expect_err("expected parse error");
        let loc = err.location().expect("location present");
        let idx = location_to_index(&src, loc);
        assert!(src.as_ref().is_char_boundary(idx));
        let e_idx = src.as_ref().find('é').expect("contains é");
        assert!(idx > e_idx, "index {idx} must follow é at {e_idx}");
        assert!(idx <= src.as_ref().len());
    }
}

#[cfg(test)]
fn expected_offset(src: &str, column: u64) -> usize {
    src.chars()
        .take(usize::try_from(column.saturating_sub(1)).unwrap_or(usize::MAX))
        .map(char::len_utf8)
        .sum()
}

#[cfg(test)]
mod byte_index_tests {
    #![allow(clippy::expect_used, reason = "tests reason about offsets")]
    use super::{byte_index_components, expected_offset};

    #[test]
    fn byte_index_accounts_for_multibyte_characters() {
        let line = "emoji: 😀value";
        let column = 9; // just after the emoji, before the 'v'.
        let offset = byte_index_components(line, 1, column);
        assert_eq!(offset, expected_offset(line, column));
    }

    #[test]
    fn byte_index_clamps_past_line_end() {
        let line = "short";
        let column = 42;
        let offset = byte_index_components(line, 1, column);
        assert_eq!(offset, line.len());
    }

    #[test]
    fn byte_index_advances_over_previous_lines() {
        let src = "one\ntwo\nthree";
        let column = 3; // 'r' in "three"
        let offset = byte_index_components(src, 3, column);
        let expected = "one\ntwo\n".len() + expected_offset("three", column);
        assert_eq!(offset, expected);
    }

    #[test]
    fn byte_index_handles_crlf_lines() {
        let src = "one\r\ntwo\r\nthree";
        let column = 2; // 'w' in "two"
        let offset = byte_index_components(src, 2, column);
        let expected = "one\r\n".len() + expected_offset("two", column);
        assert_eq!(offset, expected);
    }
}
