//! Semantic newtype wrappers for BDD step definition parameters.
//!
//! These wrappers distinguish between different string parameter types,
//! improving type safety and self-documentation in step definitions.

#![expect(
    dead_code,
    reason = "newtypes provide complete API; not all methods are used yet"
)]

use crate::bdd::fixtures::strip_quotes;
use std::fmt;
use std::path::{Path, PathBuf};

/// Generates a newtype wrapper for string parameters.
///
/// Each generated type:
/// - Wraps a `String`
/// - Strips quotes during construction via `From<String>`
/// - Provides `as_str()` and `into_string()` accessors
/// - Implements `Display` and `Debug`
macro_rules! define_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone)]
        pub struct $name(String);

        impl $name {
            /// Create a new instance, stripping surrounding quotes.
            pub fn new(s: impl Into<String>) -> Self {
                let raw = s.into();
                Self(strip_quotes(&raw).to_string())
            }

            /// Return the inner string as a slice.
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Consume the wrapper and return the inner string.
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self::new(s)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

// ---------------------------------------------------------------------------
// CLI domain types
// ---------------------------------------------------------------------------

define_newtype!(
    /// Raw CLI argument string (e.g., "build --file foo.yml").
    CliArgs
);

define_newtype!(
    /// Build target name.
    TargetName
);

define_newtype!(
    /// File or directory path string.
    PathString
);

impl PathString {
    /// Return the path as a `Path` reference.
    pub fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }

    /// Convert to an owned `PathBuf`.
    pub fn to_path_buf(&self) -> PathBuf {
        PathBuf::from(&self.0)
    }
}

define_newtype!(
    /// URL string for network policy checks.
    UrlString
);

impl UrlString {
    /// Parse the URL string into a `url::Url`.
    ///
    /// # Errors
    ///
    /// Returns an error if the string is not a valid URL.
    pub fn parse(&self) -> Result<url::Url, url::ParseError> {
        url::Url::parse(&self.0)
    }
}

define_newtype!(
    /// Error message fragment for assertion matching.
    ErrorFragment
);

// ---------------------------------------------------------------------------
// Manifest domain types
// ---------------------------------------------------------------------------

define_newtype!(
    /// Manifest file path.
    ManifestPath
);

impl ManifestPath {
    /// Return the path as a `Path` reference.
    pub fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }
}

define_newtype!(
    /// Environment variable name.
    EnvVarKey
);

define_newtype!(
    /// Environment variable value.
    EnvVarValue
);

define_newtype!(
    /// Command string from a target recipe.
    CommandText
);

define_newtype!(
    /// Script content from a target recipe.
    ScriptText
);

define_newtype!(
    /// Rule name identifier.
    RuleName
);

define_newtype!(
    /// Error pattern for matching parse errors.
    ErrorPattern
);

define_newtype!(
    /// Dependency name.
    DepName
);

define_newtype!(
    /// Source file path.
    SourcePath
);

define_newtype!(
    /// Macro signature string.
    MacroSignature
);

define_newtype!(
    /// Version string identifier.
    VersionString
);

define_newtype!(
    /// Comma-separated list of names.
    NamesList
);

impl NamesList {
    /// Split the names by comma and return an iterator of trimmed strings.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.0.split(',').map(str::trim).filter(|s| !s.is_empty())
    }

    /// Collect the names into a set.
    pub fn to_set(&self) -> std::collections::BTreeSet<String> {
        self.iter().map(str::to_string).collect()
    }
}

// ---------------------------------------------------------------------------
// Manifest command domain types
// ---------------------------------------------------------------------------

define_newtype!(
    /// Output fragment for assertions.
    OutputFragment
);

define_newtype!(
    /// Manifest command output path.
    ManifestOutputPath
);

define_newtype!(
    /// Directory name.
    DirectoryName
);

define_newtype!(
    /// File name.
    FileName
);

// ---------------------------------------------------------------------------
// Stdlib workspace domain types
// ---------------------------------------------------------------------------

define_newtype!(
    /// HTTP response body content.
    HttpResponseBody
);

define_newtype!(
    /// File contents for stdlib fixtures.
    FileContents
);

impl FileContents {
    /// Return the contents as a byte slice.
    pub const fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

define_newtype!(
    /// PATH environment variable entries (colon-separated).
    PathEntries
);

/// Command helper name (does not strip quotes).
///
/// Unlike other newtypes, this is constructed from static strings and
/// does not perform quote stripping.
#[derive(Debug, Clone)]
pub struct HelperName(&'static str);

impl HelperName {
    /// Return the helper name as a string slice.
    pub const fn as_str(&self) -> &str {
        self.0
    }
}

impl From<&'static str> for HelperName {
    fn from(s: &'static str) -> Self {
        Self(s)
    }
}

// ---------------------------------------------------------------------------
// CLI domain types (non-string)
// ---------------------------------------------------------------------------

/// Job count for parallel execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobCount(usize);

impl JobCount {
    /// Create a new job count.
    pub const fn new(count: usize) -> Self {
        Self(count)
    }

    /// Return the job count value.
    pub const fn value(self) -> usize {
        self.0
    }
}

impl From<usize> for JobCount {
    fn from(count: usize) -> Self {
        Self(count)
    }
}
