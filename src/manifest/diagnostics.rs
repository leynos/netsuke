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
#[derive(Debug, Clone)]
pub struct ManifestSource(String);

impl ManifestSource {
    #[must_use]
    pub fn new(src: impl Into<String>) -> Self {
        Self(src.into())
    }

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

/// Display name for a manifest source used in diagnostics.
///
/// # Examples
/// ```rust
/// use netsuke::manifest::ManifestName;
/// let name = ManifestName::new("Netsukefile");
/// assert_eq!(name.as_str(), "Netsukefile");
/// ```
#[derive(Debug, Clone)]
pub struct ManifestName(String);

impl ManifestName {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

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
        write!(f, "{}", self.0)
    }
}

use super::hints::YAML_HINTS;

fn saturating_usize(value: u64) -> usize {
    usize::try_from(value.min(usize::MAX as u64)).unwrap_or(usize::MAX)
}

fn location_to_index(src: &ManifestSource, loc: Location) -> usize {
    let target_line = saturating_usize(loc.line().saturating_sub(1));
    let target_column = saturating_usize(loc.column().saturating_sub(1));
    let mut offset = 0usize;
    for (idx, segment) in src.as_ref().split_inclusive('\n').enumerate() {
        if idx == target_line {
            let line = segment.strip_suffix('\n').unwrap_or(segment);
            let byte_index = line
                .char_indices()
                .nth(target_column)
                .map_or(line.len(), |(byte_idx, _)| byte_idx);
            return offset + byte_index;
        }
        offset += segment.len();
    }
    src.as_ref().len()
}

fn to_span(src: &ManifestSource, loc: Location) -> SourceSpan {
    let at = location_to_index(src, loc);
    let bytes = src.as_ref().as_bytes();
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

fn has_tab_indent(src: &ManifestSource, loc: Option<Location>) -> bool {
    let Some(loc) = loc else { return false };
    let line_idx = saturating_usize(loc.line().saturating_sub(1));
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
        src: NamedSource::new(name.as_ref(), src.as_ref().to_string()),
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

#[must_use]
pub fn map_data_error(
    err: serde_json::Error,
    name: &ManifestName,
) -> Box<dyn Diagnostic + Send + Sync + 'static> {
    let message = format!("manifest structure error in {}: {err}", name.as_ref());
    Box::new(DataDiagnostic {
        source: err,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_yaml_error_includes_tab_hint() {
        let src = ManifestSource::from("\tkey: \"unterminated");
        let err = serde_saphyr::from_str::<serde_json::Value>(src.as_ref())
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
}
