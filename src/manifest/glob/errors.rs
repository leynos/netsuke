//! Error helpers for glob processing.
use minijinja::{Error, ErrorKind};

use crate::localization::{self, keys};

#[derive(Debug)]
pub(super) struct GlobErrorContext {
    pub pattern: String,
    pub error_char: char,
    pub position: usize,
    pub error_type: GlobErrorType,
}

#[derive(Debug)]
pub(super) enum GlobErrorType {
    UnmatchedBrace,
    InvalidPattern,
    IoError,
}

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

pub(super) fn create_unmatched_brace_error(context: &GlobErrorContext) -> Error {
    create_glob_error(context, None)
}
