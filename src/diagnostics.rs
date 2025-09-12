//! Diagnostics utilities.
//!
//! Adds `.diag(...)` and `.diag_with(...)` extension methods to `Result` for
//! converting errors into `miette` diagnostics with contextual messages.
//!
//! # Examples
//! ```rust,ignore
//! use miette::Result;
//! use crate::diagnostics::ResultExt;
//!
//! fn load() -> Result<()> {
//!     std::fs::read_to_string("Netsukefile").diag("read Netsukefile")?;
//!     Ok(())
//! }
//! ```
use miette::{Context, IntoDiagnostic, Result};
use std::fmt::Display;

/// Extension methods for turning `Result` values into `miette` diagnostics.
///
/// # Examples
///
/// ```rust,ignore
/// use miette::Result;
/// use crate::diagnostics::ResultExt;
/// use std::fs::File;
///
/// fn open(path: &str) -> Result<File> {
///     File::open(path).diag("open file")
/// }
/// ```
#[expect(dead_code, reason = "unused after migration to anyhow")]
pub(crate) trait ResultExt<T> {
    /// Attach a static context message to any error.
    ///
    /// ```rust,ignore
    /// use miette::Result;
    /// use crate::diagnostics::ResultExt;
    /// use std::fs::read_to_string;
    ///
    /// fn read(path: &str) -> Result<String> {
    ///     read_to_string(path).diag("read file")
    /// }
    /// ```
    fn diag(self, context: impl Display + Send + Sync + 'static) -> Result<T>;

    /// Attach a lazily evaluated context message to any error.
    ///
    /// ```rust,ignore
    /// use miette::Result;
    /// use crate::diagnostics::ResultExt;
    /// use std::{fs::File, path::Path};
    ///
    /// fn open(path: &Path) -> Result<File> {
    ///     File::open(path).diag_with(|| format!("open {}", path.display()))
    /// }
    /// ```
    fn diag_with(self, f: impl FnOnce() -> String) -> Result<T>;
}

impl<T, E> ResultExt<T> for std::result::Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn diag(self, context: impl Display + Send + Sync + 'static) -> Result<T> {
        self.into_diagnostic().wrap_err(context)
    }

    fn diag_with(self, f: impl FnOnce() -> String) -> Result<T> {
        self.into_diagnostic().wrap_err_with(f)
    }
}
