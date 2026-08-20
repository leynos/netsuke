//! Owned Ninja manifest content shared by runner output paths.

/// Wrapper around generated Ninja manifest text.
#[derive(Debug, Clone)]
pub struct NinjaContent(String);

impl NinjaContent {
    /// Store the provided Ninja manifest string.
    #[must_use]
    pub const fn new(content: String) -> Self {
        Self(content)
    }

    /// Borrow the underlying manifest text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the wrapper, returning the owned manifest string.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}
