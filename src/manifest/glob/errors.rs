//! Error helpers for glob processing.
use minijinja::{Error, ErrorKind};

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
            format!(
                "invalid glob pattern '{}': unmatched '{}' at position {}",
                context.pattern, context.error_char, context.position
            ),
        ),
        GlobErrorType::InvalidPattern => {
            let detail = details.unwrap_or_else(|| "unknown pattern error".to_owned());
            Error::new(
                ErrorKind::SyntaxError,
                format!("invalid glob pattern '{}': {detail}", context.pattern),
            )
        }
        GlobErrorType::IoError => {
            let detail = details.unwrap_or_else(|| "unknown IO error".to_owned());
            let message = if detail.starts_with("glob ") {
                detail
            } else {
                format!("glob failed for '{}': {detail}", context.pattern)
            };
            Error::new(ErrorKind::InvalidOperation, message)
        }
    }
}

pub(super) fn create_unmatched_brace_error(context: &GlobErrorContext) -> Error {
    create_glob_error(context, None)
}
