//! YAML parsing diagnostics and helper utilities.
//!
//! This module converts `serde_saphyr` parse errors into [`miette`]
//! diagnostics with source spans and contextual hints for common YAML
//! mistakes such as tab indentation. The public entry point is
//! [`map_yaml_error`], which accepts a manifest name and source buffer to
//! generate actionable error messages for callers.

// Module-level suppression for version-dependent lint false positives from
// miette/thiserror derive macros. The unused_assignments lint fires in some
// Rust versions but not others. Since `#[expect]` fails when the lint doesn't
// fire, and `unfulfilled_lint_expectations` cannot be expected, we must use
// `#[allow]` here. FIXME: remove once upstream is fixed.
#![allow(clippy::allow_attributes, clippy::allow_attributes_without_reason)]

use super::{ManifestName, ManifestSource};
use crate::manifest::hints::YAML_HINTS;
use miette::{Diagnostic, NamedSource, SourceSpan};
use serde_saphyr::{Error as YamlError, Location};
use thiserror::Error;

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

// FIXME: remove unused_assignments suppression once miette/thiserror derive
// false positive is fixed upstream. The lint fires in some Rust versions but
// not others due to how the derive macros expand field assignments.
#[allow(unused_assignments)]
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

#[cfg(test)]
fn expected_offset(src: &str, column: u64) -> usize {
    src.chars()
        .take(usize::try_from(column.saturating_sub(1)).unwrap_or(usize::MAX))
        .map(char::len_utf8)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Context, Result, anyhow, ensure};
    use std::error::Error as StdError;

    #[test]
    fn map_yaml_error_includes_tab_hint() -> Result<()> {
        let src = ManifestSource::from("\tkey: \"unterminated");
        let Err(err) = serde_saphyr::from_str::<crate::manifest::ManifestValue>(src.as_ref())
        else {
            return Err(anyhow!(
                "expected YAML parse error for source {:?}",
                src.as_str()
            ));
        };
        let name = ManifestName::from("test");
        let diag = map_yaml_error(err, &src, &name);
        let msg = diag.to_string();
        ensure!(
            msg.contains("Use spaces for indentation"),
            "message missing tab hint: {msg}"
        );
        Ok(())
    }

    #[test]
    fn map_yaml_error_defaults_location_when_missing() -> Result<()> {
        let src = ManifestSource::from("foo: [1");
        let err = serde_saphyr::Error::Eof {
            location: serde_saphyr::Location::UNKNOWN,
        };
        let name = ManifestName::from("test");
        let diag = map_yaml_error(err, &src, &name);
        ensure!(
            diag.to_string().contains("line 1, column 1"),
            "diagnostic should default to line 1 column 1"
        );
        Ok(())
    }

    #[test]
    fn map_yaml_error_span_skips_carriage_return() -> Result<()> {
        let src = ManifestSource::from("targets:\r\n  - name: hi\r\n    command echo\r\n");
        let Err(err) = serde_saphyr::from_str::<crate::manifest::ManifestValue>(src.as_ref())
        else {
            return Err(anyhow!("expected parse error for carriage-return input"));
        };
        let name = ManifestName::from("test");
        let diag = map_yaml_error(err, &src, &name);
        let yaml_diag = (&*diag as &(dyn StdError + 'static))
            .downcast_ref::<YamlDiagnostic>()
            .ok_or_else(|| anyhow!("expected YAML diagnostic"))?;
        let span = yaml_diag.span.context("span present")?;
        let offset = span.offset();
        if let Some(byte) = src.as_ref().as_bytes().get(offset) {
            ensure!(*byte != b'\r', "span should skip carriage returns");
        }
        Ok(())
    }

    #[test]
    fn location_to_index_handles_utf8() -> Result<()> {
        // café: 'é' is multi-byte
        let src = ManifestSource::from("café: [\n");
        let Err(err) = serde_saphyr::from_str::<crate::manifest::ManifestValue>(src.as_ref())
        else {
            return Err(anyhow!("expected parse error for UTF-8 test"));
        };
        let loc = err.location().context("location present")?;
        let idx = location_to_index(&src, loc);
        ensure!(
            src.as_ref().is_char_boundary(idx),
            "index {idx} should align to char boundary"
        );
        let e_idx = src
            .as_ref()
            .find('é')
            .ok_or_else(|| anyhow!("source should contain 'é'"))?;
        ensure!(idx > e_idx, "index {idx} must follow é at {e_idx}");
        ensure!(
            idx <= src.as_ref().len(),
            "index {idx} should fall within source length {}",
            src.as_ref().len()
        );
        Ok(())
    }
}

#[cfg(test)]
mod byte_index_tests {
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
