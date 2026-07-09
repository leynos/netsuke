//! Ninja-domain and non-string CLI parameter types for BDD steps.
//!
//! Split from `types.rs` so both files stay within the module size budget.

use std::fmt;

define_newtype!(
    /// Fragment expected to appear in ninja output (e.g., "build: phony").
    NinjaFragment
);

define_newtype!(
    /// Comma-separated list of expected tokens from shlex parsing.
    TokenList,
    accessors: []
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
