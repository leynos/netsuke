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
    pub normalized: Option<String>,
}

pub type GlobEntryResult = std::result::Result<std::path::PathBuf, glob::GlobError>;

/// Expand a glob pattern and collect the matching UTF-8 file paths.
///
/// # Errors
///
/// Returns an error when the pattern is syntactically invalid, when
/// capability-restricted filesystem access fails, or when a match contains
/// non-UTF-8 data.
///
/// # Panics
///
/// Panics if pattern normalisation fails to record the derived pattern, which
/// indicates a logic error in the validator.
pub fn glob_paths(pattern: &str) -> std::result::Result<Vec<String>, Error> {
    use glob::{MatchOptions, glob_with};

    let opts = MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: false,
    };

    let mut pattern_state = GlobPattern {
        raw: pattern.to_owned(),
        normalized: None,
    };

    validate_brace_matching(&pattern_state)?;

    #[cfg(unix)]
    let mut normalized = normalize_separators(&pattern_state.raw);
    #[cfg(not(unix))]
    let normalized = normalize_separators(&pattern_state.raw);
    #[cfg(unix)]
    {
        normalized = force_literal_escapes(&normalized);
    }

    pattern_state.normalized = Some(normalized);
    let normalized_pattern = pattern_state.normalized.as_deref().ok_or_else(|| {
        create_glob_error(
            &GlobErrorContext {
                pattern: pattern_state.raw.clone(),
                error_char: char::from(0),
                position: 0,
                error_type: GlobErrorType::InvalidPattern,
            },
            Some("pattern normalisation missing after validation".to_owned()),
        )
    })?;

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

    let entries = glob_with(normalized_pattern, opts).map_err(|e| {
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
        if let Some(p) = process_glob_entry(entry, pattern_state.clone(), &root)? {
            paths.push(p);
        }
    }
    Ok(paths)
}

#[cfg(test)]
mod tests;
