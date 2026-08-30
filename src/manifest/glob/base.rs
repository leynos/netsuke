//! Canonicalize injected bases before relative glob compilation.
//!
//! This module owns only the filesystem-to-path conversion at the injected
//! base seam. Pattern preparation owns joining and escaping the resulting
//! path, while the walker owns opening its literal prefix.

use super::{
    GlobPattern,
    errors::{GlobErrorContext, GlobErrorType, create_glob_error},
    escape::escape_glob_literal_path,
};
use camino::{Utf8Path, Utf8PathBuf};
use minijinja::Error;
use std::sync::{Mutex, MutexGuard};

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

/// Cache a manifest parse's injected glob base after its first relative use.
///
/// This type belongs only to the manifest parse boundary. Direct
/// [`super::glob_paths`] callers continue to provide their optional base per
/// query; Jinja's closure owns one cache so multiple relative `glob()` calls
/// do not repeat filesystem canonicalization.
pub(in crate::manifest) struct GlobBaseCache {
    /// Base supplied by the manifest workspace, before filesystem preparation.
    base: Option<Utf8PathBuf>,
    /// Successfully canonicalized base retained for the rest of the parse.
    resolved: Mutex<Option<Utf8PathBuf>>,
}

impl GlobBaseCache {
    /// Create an empty cache around an optional manifest workspace base.
    pub(in crate::manifest) const fn new(base: Option<Utf8PathBuf>) -> Self {
        Self {
            base,
            resolved: Mutex::new(None),
        }
    }

    /// Resolve and retain the configured base when one is available.
    ///
    /// # Errors
    ///
    /// Returns the canonicalization error from the injected base on its first
    /// relative use.
    fn resolve(&self) -> std::result::Result<Option<Utf8PathBuf>, Error> {
        let Some(base) = self.base.as_deref() else {
            return Ok(None);
        };
        let cached = self.lock_resolved(base)?.clone();
        if cached.is_some() {
            return Ok(cached);
        }

        let canonical = resolve_relative_glob_base(base)?;
        let mut resolved = self.lock_resolved(base)?;
        if let Some(published) = resolved.as_ref() {
            return Ok(Some(published.clone()));
        }
        resolved.replace(canonical.clone());
        Ok(Some(canonical))
    }

    /// Lock the resolved-base cache and preserve a contextual poisoning error.
    fn lock_resolved(
        &self,
        base: &Utf8Path,
    ) -> std::result::Result<MutexGuard<'_, Option<Utf8PathBuf>>, Error> {
        self.resolved.lock().map_err(|error| {
            create_glob_error(
                &GlobErrorContext {
                    pattern: base.to_string(),
                    error_char: char::from(0),
                    position: 0,
                    error_type: GlobErrorType::IoError,
                },
                Some(format!("manifest glob-base cache lock poisoned: {error}")),
            )
        })
    }
}

/// Hold a validated glob pattern, search text, and optional rebase path.
///
/// Pattern preparation lives beside base resolution because both constructors
/// decide whether a relative pattern needs a filesystem-prepared base.
pub(super) struct PreparedGlob {
    /// Validated pattern and its normalised spelling.
    pub(super) pattern: GlobPattern,
    /// Search text handed to `glob_with`, with a resolved base when relative.
    pub(super) search: String,
    /// Canonicalized base stripped from matches, present exactly when relative.
    pub(super) strip: Option<Utf8PathBuf>,
}

impl PreparedGlob {
    /// Prepare `pattern` and any relative injected `base` for filesystem matching.
    ///
    /// Canonicalizes a relative injected base through
    /// [`resolve_relative_glob_base`] before embedding its escaped literal
    /// spelling into the search text.
    ///
    /// # Errors
    ///
    /// Returns an error when the pattern fails brace validation or when the
    /// injected base cannot be canonicalized.
    pub(super) fn new(pattern: &str, base: Option<&Utf8Path>) -> std::result::Result<Self, Error> {
        let pattern_state = GlobPattern::new(pattern)?;
        let normalized = pattern_state.normalized();
        let resolved_base = match base {
            Some(dir) if !Utf8Path::new(normalized).is_absolute() => {
                Some(resolve_relative_glob_base(dir)?)
            }
            _ => None,
        };
        Ok(Self::from_pattern_and_base(pattern_state, resolved_base))
    }

    /// Prepare `pattern` using a manifest parse's cached injected base.
    ///
    /// # Errors
    ///
    /// Returns an error when the pattern fails brace validation or its first
    /// relative use cannot canonicalize the injected base.
    pub(super) fn new_with_base_cache(
        pattern: &str,
        base: &GlobBaseCache,
    ) -> std::result::Result<Self, Error> {
        let pattern_state = GlobPattern::new(pattern)?;
        let resolved_base = (!Utf8Path::new(pattern_state.normalized()).is_absolute())
            .then(|| base.resolve())
            .transpose()?
            .flatten();
        Ok(Self::from_pattern_and_base(pattern_state, resolved_base))
    }

    /// Build glob search text from a validated pattern and resolved base.
    fn from_pattern_and_base(pattern: GlobPattern, base: Option<Utf8PathBuf>) -> Self {
        let (search, strip) = base.map_or_else(
            || (pattern.normalized().to_owned(), None),
            |dir| {
                let escaped = escape_glob_literal_path(&dir);
                let separator = std::path::MAIN_SEPARATOR;
                (
                    format!("{escaped}{separator}{}", pattern.normalized()),
                    Some(dir),
                )
            },
        );
        Self {
            pattern,
            search,
            strip,
        }
    }
}
