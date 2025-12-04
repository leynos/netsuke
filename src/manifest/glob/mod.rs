//! Utilities for normalising and validating manifest glob patterns.
use minijinja::Error;

mod errors;
mod normalize;
mod validate;
mod walk;

use errors::{GlobErrorContext, GlobErrorType, create_glob_error};
use normalize::normalize_separators;
use validate::validate_brace_matching;
use walk::{open_root_dir, process_glob_entry};

#[cfg(unix)]
use normalize::force_literal_escapes;

#[derive(Debug, Clone)]
/// A glob pattern and its normalised representation.
pub struct GlobPattern {
    raw: String,
    normalized: String,
}

impl GlobPattern {
    /// Access the pattern as provided by the caller.
    #[must_use]
    #[expect(
        clippy::missing_const_for_fn,
        reason = "const String::as_str() not available on all MSRV targets"
    )]
    pub fn raw(&self) -> &str {
        self.raw.as_str()
    }

    /// Access the platform-normalised pattern suitable for globbing.
    #[must_use]
    #[expect(
        clippy::missing_const_for_fn,
        reason = "const String::as_str() not available on all MSRV targets"
    )]
    pub fn normalized(&self) -> &str {
        self.normalized.as_str()
    }

    /// Validate and normalise a glob pattern, preventing inconsistent state.
    ///
    /// # Errors
    ///
    /// Returns an error when brace validation fails.
    pub fn new(raw: &str) -> std::result::Result<Self, Error> {
        validate_brace_matching(raw)?;

        #[cfg(unix)]
        let normalized = {
            let normalized = normalize_separators(raw);
            force_literal_escapes(&normalized)
        };
        #[cfg(not(unix))]
        let normalized = normalize_separators(raw);

        Ok(Self {
            raw: raw.to_owned(),
            normalized,
        })
    }
}

pub type GlobEntryResult = std::result::Result<std::path::PathBuf, glob::GlobError>;

/// Expand a glob pattern and collect the matching UTF-8 file paths.
///
/// # Errors
///
/// Returns an error when the pattern is syntactically invalid, when
/// capability-restricted filesystem access fails, or when a match contains
/// non-UTF-8 data.
pub fn glob_paths(pattern: &str) -> std::result::Result<Vec<String>, Error> {
    use glob::{MatchOptions, glob_with};

    let opts = MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: false,
    };

    let pattern_state = GlobPattern::new(pattern)?;

    let root = open_root_dir(&pattern_state).map_err(|e| {
        create_glob_error(
            &GlobErrorContext {
                pattern: pattern_state.raw().to_owned(),
                error_char: char::from(0),
                position: 0,
                error_type: GlobErrorType::IoError,
            },
            Some(e.to_string()),
        )
    })?;

    let entries = glob_with(pattern_state.normalized(), opts).map_err(|e| {
        create_glob_error(
            &GlobErrorContext {
                pattern: pattern_state.raw().to_owned(),
                error_char: char::from(0),
                position: 0,
                error_type: GlobErrorType::InvalidPattern,
            },
            Some(e.to_string()),
        )
    })?;
    let mut paths = Vec::new();
    for entry in entries {
        if let Some(p) = process_glob_entry(entry, &pattern_state, &root)? {
            paths.push(p);
        }
    }
    Ok(paths)
}

#[cfg(test)]
mod tests;
