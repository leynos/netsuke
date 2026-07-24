//! Semantic newtype wrappers for BDD step definition parameters.
//!
//! These wrappers distinguish between different string parameter types,
//! improving type safety and self-documentation in step definitions.

use std::fmt;
use std::path::Path;

/// Generates a newtype wrapper for string parameters.
///
/// Each generated type wraps a `String` and implements `Debug`, `Clone`,
/// `Display`, `From<String>`, `From<&str>`, `FromStr`, and `AsRef<str>`.
///
/// Inherent string accessors are opt-in per type so no generated method is ever
/// dead code: the bare form `define_newtype!(Name)` emits `as_str` (the common
/// case), while `define_newtype!(Name, accessors: [..])` emits exactly the
/// listed accessors (`as_str`, `into_string`, or neither via `[]`).
macro_rules! define_newtype {
    (@accessor as_str) => {
        /// Return the inner string as a slice.
        pub fn as_str(&self) -> &str {
            &self.0
        }
    };
    (@accessor into_string) => {
        /// Consume the wrapper and return the inner string.
        pub fn into_string(self) -> String {
            self.0
        }
    };

    ($(#[$meta:meta])* $name:ident) => {
        define_newtype!($(#[$meta])* $name, accessors: [as_str]);
    };

    ($(#[$meta:meta])* $name:ident, accessors: [$($acc:ident),* $(,)?]) => {
        $(#[$meta])*
        #[derive(Debug, Clone)]
        pub struct $name(String);

        impl $name {
            /// Create a new instance.
            pub fn new(s: impl Into<String>) -> Self {
                Self(s.into())
            }

            $(define_newtype!(@accessor $acc);)*
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
    PathString,
    accessors: []
);

impl PathString {
    /// Return the path as a `Path` reference.
    pub fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }
}

impl AsRef<Path> for PathString {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

define_newtype!(
    /// URL string for network policy checks.
    UrlString,
    accessors: []
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

impl AsRef<Path> for ManifestPath {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

define_newtype!(
    /// Environment variable name.
    EnvVarKey,
    accessors: [as_str, into_string]
);

define_newtype!(
    /// Environment variable value.
    EnvVarValue,
    accessors: [as_str, into_string]
);

define_newtype!(
    /// Command string from a target recipe.
    CommandText
);

define_newtype!(
    /// Script content from a target recipe.
    ScriptText,
    accessors: []
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
    MacroSignature,
    accessors: []
);

define_newtype!(
    /// Version string identifier.
    VersionString
);

define_newtype!(
    /// Comma-separated list of names.
    NamesList,
    accessors: []
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
    ContextValue,
    accessors: [into_string]
);

define_newtype!(
    /// HTTP response body content.
    HttpResponseBody,
    accessors: [into_string]
);

define_newtype!(
    /// File contents for stdlib fixtures.
    FileContents,
    accessors: []
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
// Ninja domain and non-string CLI types
// ---------------------------------------------------------------------------

#[path = "types_ninja.rs"]
mod types_ninja;
pub use types_ninja::{ContentName, JobCount, NinjaFragment, TokenList};
