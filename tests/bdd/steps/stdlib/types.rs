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
