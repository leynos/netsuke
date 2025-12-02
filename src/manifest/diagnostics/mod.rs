//! Translates manifest parsing errors into actionable diagnostics.
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
/// use miette::miette;
/// use netsuke::manifest::ManifestError;
///
/// let err = ManifestError::Parse { source: Box::new(miette!("bad manifest")) };
/// assert_eq!(format!("{err}"), "manifest parse error");
/// ```
#[derive(Debug, Error, Diagnostic)]
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

#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
#[diagnostic(code(netsuke::manifest::structure))]
struct DataDiagnostic {
    #[source]
    source: serde_json::Error,
    message: String,
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
    let message = format!(
        "[netsuke::manifest::structure] manifest structure error in {}: {err}",
        name.as_ref()
    );
    Box::new(DataDiagnostic {
        source: err,
        message,
    })
}
