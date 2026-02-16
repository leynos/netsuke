//! Semantic newtype wrappers for BDD step definition parameters.
//!
//! These wrappers distinguish between different string parameter types,
//! improving type safety and self-documentation in step definitions.

// The `define_newtype` macro generates `as_str` and `into_string` methods for
// API completeness. Some types use all methods while others don't. Using
// `#[expect(dead_code)]` is not feasible because it requires the lint to fire,
// which varies per type instantiation. The `#[allow(dead_code)]` within the
// macro requires this module-level exception to `clippy::allow_attributes`.
#![expect(
    clippy::allow_attributes,
    reason = "macro-generated dead_code suppression varies per type instantiation"
)]

use std::fmt;
use std::path::{Path, PathBuf};

/// Generates a newtype wrapper for string parameters.
///
/// Each generated type:
/// - Wraps a `String`
/// - Provides `as_str()` and `into_string()` accessors
/// - Implements `Display` and `Debug`
macro_rules! define_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone)]
        pub struct $name(String);

        impl $name {
            /// Create a new instance.
            pub fn new(s: impl Into<String>) -> Self {
                Self(s.into())
            }

            /// Return the inner string as a slice.
            #[allow(dead_code, reason = "newtype provides complete API; usage varies per type")]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Consume the wrapper and return the inner string.
            #[allow(dead_code, reason = "newtype provides complete API; usage varies per type")]
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self::new(s)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self::new(s)
            }
        }

        impl std::str::FromStr for $name {
            type Err = std::convert::Infallible;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self::new(s))
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

impl AsRef<Path> for PathString {
    fn as_ref(&self) -> &Path {
        self.as_path()
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

    /// Convert to an owned `PathBuf`.
    #[allow(dead_code, reason = "path wrapper provides complete API; usage varies")]
    pub fn to_path_buf(&self) -> PathBuf {
        PathBuf::from(&self.0)
    }
}

impl AsRef<Path> for ManifestPath {
    fn as_ref(&self) -> &Path {
        self.as_path()
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
        self.iter().map(str::to_owned).collect()
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
    /// Template content string for rendering.
    ///
    /// Distinguishes template source from other string parameters in rendering
    /// functions, improving type safety and API clarity.
    TemplateContent
);

define_newtype!(
    /// Template context variable key.
    ContextKey
);

define_newtype!(
    /// Template context variable value.
    ContextValue
);

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

define_newtype!(
    /// URL scheme for network policy configuration (e.g., "https").
    Scheme
);

define_newtype!(
    /// Host name for network policy configuration.
    HostName
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
// Ninja domain types
// ---------------------------------------------------------------------------

define_newtype!(
    /// Fragment expected to appear in ninja output (e.g., "build: phony").
    NinjaFragment
);

define_newtype!(
    /// Comma-separated list of expected tokens from shlex parsing.
    TokenList
);

impl TokenList {
    /// Parse the token list into a Vec of strings, replacing escaped newlines.
    pub fn to_vec(&self) -> Vec<String> {
        self.0
            .split(',')
            .map(|w| w.trim().replace("\\n", "\n"))
            .collect()
    }
}

/// Content name for error messages in ninja-related assertions.
#[derive(Debug, Clone, Copy)]
pub enum ContentName {
    /// Ninja file content.
    NinjaContent,
    /// Ninja generation error.
    NinjaError,
}

impl ContentName {
    /// Return the content name as a static string slice.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NinjaContent => "ninja content",
            Self::NinjaError => "ninja error",
        }
    }
}

impl fmt::Display for ContentName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
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
