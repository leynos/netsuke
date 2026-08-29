//! Canonicalize injected bases before relative glob compilation.
//!
//! This module owns only the filesystem-to-path conversion at the injected
//! base seam. Pattern preparation owns joining and escaping the resulting
//! path, while the walker owns opening its literal prefix.

use super::errors::{GlobErrorContext, GlobErrorType, create_glob_error};
use camino::{Utf8Path, Utf8PathBuf};
use minijinja::Error;

/// Resolve an injected base for a relative pattern to a canonical UTF-8 path.
///
/// A workspace reached through a symbolic link must still expand relative
/// globs. `dunce` retains canonicalization while simplifying safe Windows
/// verbatim disk prefixes, which the `glob` crate deliberately does not
/// enumerate.
///
/// # Errors
///
/// Propagates canonicalization and UTF-8 conversion failures as
/// [`GlobErrorType::IoError`].
pub(super) fn resolve_relative_glob_base(
    base: &Utf8Path,
) -> std::result::Result<Utf8PathBuf, Error> {
    let canonical = dunce::canonicalize(base.as_std_path())
        .map_err(|error| create_base_error(base, error.to_string()))?;
    Utf8PathBuf::from_path_buf(canonical).map_err(|path| {
        create_base_error(
            base,
            format!("canonical base path is not valid UTF-8: {}", path.display()),
        )
    })
}

/// Build a glob I/O error describing an unusable injected base.
fn create_base_error(base: &Utf8Path, detail: String) -> Error {
    create_glob_error(
        &GlobErrorContext {
            pattern: base.to_string(),
            error_char: char::from(0),
            position: 0,
            error_type: GlobErrorType::IoError,
        },
        Some(detail),
    )
}
