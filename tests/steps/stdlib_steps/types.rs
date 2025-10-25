//! Test-only wrapper types that provide lightweight structure around stdlib
//! rendering inputs and outputs, keeping step definitions ergonomic while
//! preserving invariants such as UTF-8 paths.
use camino::{Utf8Path, Utf8PathBuf};

/// Owned UTF-8 path used by stdlib step definitions for workspace templates.
#[derive(Debug, Clone)]
pub(crate) struct TemplatePath(pub(crate) Utf8PathBuf);

impl TemplatePath {
    /// Borrow the path as an [`Utf8Path`] slice.
    pub(crate) fn as_path(&self) -> &Utf8Path {
        &self.0
    }
}

impl AsRef<Utf8Path> for TemplatePath {
    fn as_ref(&self) -> &Utf8Path {
        self.as_path()
    }
}

impl From<String> for TemplatePath {
    fn from(value: String) -> Self {
        Self(Utf8PathBuf::from(value))
    }
}

impl From<Utf8PathBuf> for TemplatePath {
    fn from(value: Utf8PathBuf) -> Self {
        Self(value)
    }
}

impl From<&Utf8Path> for TemplatePath {
    fn from(value: &Utf8Path) -> Self {
        Self(value.to_path_buf())
    }
}

impl From<&Utf8PathBuf> for TemplatePath {
    fn from(value: &Utf8PathBuf) -> Self {
        Self(value.clone())
    }
}

impl From<&str> for TemplatePath {
    fn from(value: &str) -> Self {
        Self(Utf8PathBuf::from(value))
    }
}

/// Owned template source used by stdlib rendering steps.
#[derive(Debug, Clone)]
pub(crate) struct TemplateContent(String);

impl TemplateContent {
    /// Borrow the underlying template text.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for TemplateContent {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<String> for TemplateContent {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for TemplateContent {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

/// Owned file body used when materialising stdlib fixtures.
#[derive(Debug, Clone)]
pub(crate) struct FileContent(String);

impl FileContent {
    /// Borrow the underlying bytes for writing to disk.
    pub(crate) const fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl AsRef<[u8]> for FileContent {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl From<String> for FileContent {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for FileContent {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

/// Owned relative path string rooted at the temporary stdlib workspace.
#[derive(Debug, Clone)]
pub(crate) struct RelativePath(String);

impl RelativePath {
    /// Borrow the relative path as a string slice.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn to_path_buf(&self) -> Utf8PathBuf {
        Utf8PathBuf::from(self.as_str())
    }
}

impl From<String> for RelativePath {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for RelativePath {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl AsRef<str> for RelativePath {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
