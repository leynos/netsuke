//! Translates manifest parsing errors into actionable diagnostics.
use miette::{Diagnostic, NamedSource, SourceSpan};
use serde_yml::{Error as YamlError, Location};
use thiserror::Error;

use super::hints::YAML_HINTS;

fn to_span(src: &str, loc: Location) -> SourceSpan {
    let at = loc.index();
    let bytes = src.as_bytes();
    let (start, end) = match bytes.get(at) {
        Some(&b) if b != b'\n' => (at, at + 1),
        _ => {
            let start = if at > 0 && bytes.get(at - 1).is_some_and(|p| *p != b'\n') {
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

fn has_tab_indent(src: &str, loc: Option<Location>) -> bool {
    let Some(loc) = loc else { return false };
    let line_idx = loc.line().saturating_sub(1);
    let line = src.lines().nth(line_idx).unwrap_or("");
    line.chars()
        .take_while(|c| c.is_whitespace())
        .any(|c| c == '\t')
}

fn hint_for(err_str: &str, src: &str, loc: Option<Location>) -> Option<String> {
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
    #[error("manifest parse error")]
    #[diagnostic(code(netsuke::manifest::parse))]
    Parse {
        #[source]
        #[diagnostic_source]
        source: Box<dyn Diagnostic + Send + Sync + 'static>,
    },
}

#[must_use]
pub fn map_yaml_error(
    err: YamlError,
    src: &str,
    name: &str,
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
        src: NamedSource::new(name, src.to_string()),
        span,
        help: hint,
        source: err,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_yaml_error_includes_tab_hint() {
        let src = "\tkey: value\n";
        let err = serde_yml::from_str::<serde_yml::Value>(src).expect_err("expected parse error");
        let diag = map_yaml_error(err, src, "test");
        let msg = diag.to_string();
        assert!(msg.contains("Use spaces for indentation"), "message: {msg}");
    }

    #[test]
    fn map_yaml_error_defaults_location_when_missing() {
        let src = ":";
        let err = serde_yml::from_str::<serde_yml::Value>(src).expect_err("expected parse error");
        let diag = map_yaml_error(err, src, "test");
        assert!(diag.to_string().contains("line 1, column 1"));
    }
}
