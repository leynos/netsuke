//! Utilities for normalising and validating manifest glob patterns.
use minijinja::Error;

mod errors;
mod normalize;
mod validate;
mod walk;

use errors::{GlobErrorContext, GlobErrorType, create_glob_error, create_unmatched_brace_error};
use normalize::normalize_separators;
use validate::validate_brace_matching;
use walk::{open_root_dir, process_glob_entry};

#[cfg(unix)]
use normalize::force_literal_escapes;

#[derive(Debug, Clone)]
pub struct GlobPattern {
    pub raw: String,
    pub normalized: String,
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

    validate_brace_matching(pattern)?;

    #[cfg(unix)]
    let mut normalized = normalize_separators(pattern);
    #[cfg(not(unix))]
    let normalized = normalize_separators(pattern);
    #[cfg(unix)]
    {
        normalized = force_literal_escapes(&normalized);
    }

    let pattern_state = GlobPattern {
        raw: pattern.to_owned(),
        normalized,
    };

    let root = open_root_dir(&pattern_state).map_err(|e| {
        create_glob_error(
            &GlobErrorContext {
                pattern: pattern_state.raw.clone(),
                error_char: char::from(0),
                position: 0,
                error_type: GlobErrorType::IoError,
            },
            Some(e.to_string()),
        )
    })?;

    let entries = glob_with(&pattern_state.normalized, opts).map_err(|e| {
        create_glob_error(
            &GlobErrorContext {
                pattern: pattern_state.raw.clone(),
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
