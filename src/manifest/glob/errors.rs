//! Error helpers for glob processing.
use minijinja::{Error, ErrorKind};

use crate::localization::{self, keys};

/// Context describing a glob pattern failure.
#[derive(Debug)]
pub(super) struct GlobErrorContext {
    /// Raw pattern that failed to expand.
    pub(super) pattern: String,
    /// Offending character, when the failure names one.
    pub(super) error_char: char,
    /// Byte position of the offending character within the pattern.
    pub(super) position: usize,
    /// Classification of the failure.
    pub(super) error_type: GlobErrorType,
}

/// Classification of a glob pattern failure.
#[derive(Debug)]
pub(super) enum GlobErrorType {
    /// A brace without its matching counterpart.
    UnmatchedBrace,
    /// Pattern syntax that the glob matcher rejects.
    InvalidPattern,
    /// A filesystem operation failed while expanding the pattern.
    IoError,
}

/// Build a `minijinja` error for a glob context with an optional detail string.
pub(super) fn create_glob_error(context: &GlobErrorContext, details: Option<String>) -> Error {
    match context.error_type {
        GlobErrorType::UnmatchedBrace => Error::new(
            ErrorKind::SyntaxError,
            localization::message(keys::MANIFEST_GLOB_UNMATCHED_BRACE)
                .with_arg("pattern", &context.pattern)
                .with_arg("character", context.error_char)
                .with_arg("position", context.position)
                .to_string(),
        ),
        GlobErrorType::InvalidPattern => {
            let detail = details.unwrap_or_else(|| {
                localization::message(keys::MANIFEST_GLOB_UNKNOWN_PATTERN_ERROR).to_string()
            });
            Error::new(
                ErrorKind::SyntaxError,
                localization::message(keys::MANIFEST_GLOB_INVALID_PATTERN)
                    .with_arg("pattern", &context.pattern)
                    .with_arg("detail", detail)
                    .to_string(),
            )
        }
        GlobErrorType::IoError => {
            let detail = details.unwrap_or_else(|| {
                localization::message(keys::MANIFEST_GLOB_UNKNOWN_IO_ERROR).to_string()
            });
            Error::new(
                ErrorKind::InvalidOperation,
                localization::message(keys::MANIFEST_GLOB_IO_FAILED)
                    .with_arg("pattern", &context.pattern)
                    .with_arg("detail", detail)
                    .to_string(),
            )
        }
    }
}

/// Build the localized unmatched-brace error for a glob context.
pub(super) fn create_unmatched_brace_error(context: &GlobErrorContext) -> Error {
    create_glob_error(context, None)
}
