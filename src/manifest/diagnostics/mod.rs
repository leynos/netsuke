//! Translates manifest parsing errors into actionable diagnostics.
//!
//! This module wraps raw parser outputs in domain-friendly types:
//! [`ManifestSource`] retains the YAML content, [`ManifestName`] labels the
//! origin, and mapping helpers (e.g. [`map_yaml_error`], [`map_data_error`])
//! convert parser and deserialisation failures into rich [`miette`]
//! diagnostics with spans, hints, and stable diagnostic codes.
//
// Module-level suppression for version-dependent lint false positives from
// miette/thiserror derive macros. The unused_assignments lint fires in some
// Rust versions but not others. Since `#[expect]` fails when the lint doesn't
// fire, and `unfulfilled_lint_expectations` cannot be expected, we must use
// `#[allow]` here. FIXME: remove once upstream is fixed.
#![allow(
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    unused_assignments
)]

use crate::localization::{self, LocalizedMessage, keys};
use miette::Diagnostic;
use thiserror::Error;

mod yaml;

pub use yaml::map_yaml_error;

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
    pub const fn as_str(&self) -> &str {
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
    pub const fn as_str(&self) -> &str {
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

/// Error raised when manifest parsing fails.
///
/// # Examples
/// ```rust
/// use miette::MietteDiagnostic;
/// use netsuke::manifest::ManifestError;
/// use netsuke::localization::{self, keys};
///
/// let msg = localization::message(keys::MANIFEST_PARSE);
/// let err = ManifestError::Parse {
///     source: Box::new(MietteDiagnostic::new("bad manifest")),
///     message: msg.clone(),
/// };
/// // Match on the variant to verify fields without asserting on message text
/// if let ManifestError::Parse { message, .. } = &err {
///     assert_eq!(message.to_string(), msg.to_string());
/// }
/// ```
#[derive(Debug, Error, Diagnostic)]
pub enum ManifestError {
    /// Manifest parsing failed and produced the supplied diagnostic.
    #[error("{message}")]
    #[diagnostic(code(netsuke::manifest::parse))]
    Parse {
        /// Underlying diagnostic reported by the parser or validator.
        #[source]
        #[diagnostic_source]
        source: Box<dyn Diagnostic + Send + Sync + 'static>,
        /// Localized parse summary.
        message: LocalizedMessage,
    },
}

#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
#[diagnostic(code(netsuke::manifest::structure))]
struct DataDiagnostic {
    #[source]
    source: serde_json::Error,
    message: LocalizedMessage,
}

/// Map a [`serde_json`] structural error into a diagnostic without a source
/// span. `serde_json` does not report byte offsets for data validation
/// failures, so the resulting diagnostic only carries the manifest name and
/// error message.
#[must_use]
pub fn map_data_error(
    err: serde_json::Error,
    name: &ManifestName,
) -> Box<dyn Diagnostic + Send + Sync + 'static> {
    let message = localization::message(keys::MANIFEST_STRUCTURE_ERROR)
        .with_arg("name", name.as_ref())
        .with_arg("details", err.to_string());
    Box::new(DataDiagnostic {
        source: err,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Context, Result, ensure};
    use miette::Diagnostic;
    use serde_json::Value;
    use test_support::{localizer_test_lock, set_en_localizer};

    #[test]
    fn map_data_error_formats_message_and_code() -> Result<()> {
        let _lock = localizer_test_lock();
        let _guard = set_en_localizer();
        let name = ManifestName::new("test.json");
        let err = serde_json::from_str::<Value>("{\"key\":}")
            .expect_err("expected serde_json parse error");
        let details = err.to_string();
        let diag = map_data_error(err, &name);
        let message = diag.to_string();
        let expected = localization::message(keys::MANIFEST_STRUCTURE_ERROR)
            .with_arg("name", "test.json")
            .with_arg("details", details)
            .to_string();
        ensure!(message == expected, "unexpected message: {message}");
        let code = diag
            .code()
            .map(|c| c.to_string())
            .context("structure diagnostic should expose a code")?;
        ensure!(
            code == "netsuke::manifest::structure",
            "unexpected diagnostic code {code}"
        );
        Ok(())
    }

    #[test]
    fn map_data_error_is_wrapped_by_manifest_error() -> Result<()> {
        let _lock = localizer_test_lock();
        let _guard = set_en_localizer();
        let name = ManifestName::new("example");
        let err = serde_json::from_str::<Value>("not json")
            .expect_err("expected serde_json parse failure");
        let diag = map_data_error(err, &name);
        let wrapped = ManifestError::Parse {
            source: diag,
            message: localization::message(keys::MANIFEST_PARSE),
        };
        let expected = localization::message(keys::MANIFEST_PARSE).to_string();
        ensure!(
            wrapped.to_string() == expected,
            "unexpected outer error message: {wrapped}"
        );
        let parse_code = wrapped
            .code()
            .map(|c| c.to_string())
            .context("parse diagnostic should expose a code")?;
        ensure!(
            parse_code == "netsuke::manifest::parse",
            "unexpected parse diagnostic code {parse_code}"
        );
        let inner_code = match &wrapped {
            ManifestError::Parse { source, .. } => source
                .code()
                .map(|c| c.to_string())
                .context("source diagnostic should have a code")?,
        };
        ensure!(
            inner_code == "netsuke::manifest::structure",
            "unexpected inner diagnostic code {inner_code}"
        );
        Ok(())
    }
}
