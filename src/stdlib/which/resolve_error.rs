//! Typed resolver errors for the `which` stdlib helpers.

use std::{fmt, io};

use camino::Utf8PathBuf;
use minijinja::{Error, ErrorKind};

use crate::localization::{self, LocalizedMessage, keys};

use super::{format_path_for_output, options::CwdMode};

const NOT_FOUND_CODE: &str = "netsuke::jinja::which::not_found";

#[derive(Debug)]
pub(crate) enum ResolveError {
    NotFound {
        command: String,
        dirs: Vec<Utf8PathBuf>,
        cwd_mode: CwdMode,
    },
    DirectNotFound {
        command: String,
        path: Utf8PathBuf,
    },
    Args {
        detail: String,
    },
    Canonicalise {
        path: Utf8PathBuf,
        source: io::Error,
    },
    CanonicaliseNonUtf8,
    WorkspaceNonUtf8 {
        command: String,
        path: String,
    },
    CwdResolve {
        source: io::Error,
    },
    CwdNonUtf8,
}

impl ResolveError {
    /// Construct an argument error for invalid or unexpected resolver options.
    ///
    /// `detail` is a human-readable explanation included in the localised
    /// diagnostic.
    pub(super) fn args(detail: impl fmt::Display) -> Self {
        Self::Args {
            detail: detail.to_string(),
        }
    }
}

impl From<ResolveError> for Error {
    fn from(value: ResolveError) -> Self {
        let (kind, message) = resolve_error_parts(&value);
        Self::new(kind, message)
    }
}

impl fmt::Display for ResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&render_message(self))
    }
}

impl std::error::Error for ResolveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Canonicalise { source, .. } | Self::CwdResolve { source } => Some(source),
            Self::NotFound { .. }
            | Self::DirectNotFound { .. }
            | Self::Args { .. }
            | Self::CanonicaliseNonUtf8
            | Self::WorkspaceNonUtf8 { .. }
            | Self::CwdNonUtf8 => None,
        }
    }
}

fn render_message(error: &ResolveError) -> String {
    resolve_error_parts(error).1
}

fn resolve_error_parts(error: &ResolveError) -> (ErrorKind, String) {
    match error {
        ResolveError::NotFound {
            command,
            dirs,
            cwd_mode,
        } => (
            ErrorKind::InvalidOperation,
            with_not_found_code(&not_found_message(command, dirs, *cwd_mode)),
        ),
        ResolveError::DirectNotFound { command, path } => (
            ErrorKind::InvalidOperation,
            with_not_found_code(&direct_not_found_message(command, path)),
        ),
        ResolveError::Args { detail } => (ErrorKind::InvalidOperation, args_message(detail)),
        ResolveError::Canonicalise { path, source } => (
            ErrorKind::InvalidOperation,
            canonicalise_message(path, source),
        ),
        ResolveError::CanonicaliseNonUtf8 => (
            ErrorKind::InvalidOperation,
            localization::message(keys::STDLIB_WHICH_CANONICALISE_NON_UTF8).to_string(),
        ),
        ResolveError::WorkspaceNonUtf8 { command, path } => (
            ErrorKind::InvalidOperation,
            workspace_non_utf8_message(command, path),
        ),
        ResolveError::CwdResolve { source } => (
            ErrorKind::InvalidOperation,
            localization::message(keys::STDLIB_WHICH_CWD_RESOLVE_FAILED)
                .with_arg("details", source.to_string())
                .to_string(),
        ),
        ResolveError::CwdNonUtf8 => (
            ErrorKind::InvalidOperation,
            localization::message(keys::STDLIB_WHICH_CWD_NON_UTF8).to_string(),
        ),
    }
}

fn with_not_found_code(message: &str) -> String {
    format!("{NOT_FOUND_CODE}: {message}")
}

/// Construct a PATH-search miss after all candidate directories are exhausted.
///
/// `command` is the lookup key, `dirs` is the searched directory set, and
/// `mode` records how the current directory contributed to the search.
pub(super) fn not_found(command: &str, dirs: &[Utf8PathBuf], mode: CwdMode) -> ResolveError {
    ResolveError::NotFound {
        command: command.to_owned(),
        dirs: dirs.to_vec(),
        cwd_mode: mode,
    }
}

/// Construct a direct-path miss for a path-like command lookup.
///
/// Use this when the command names a filesystem path rather than a PATH search;
/// `path` is the resolved candidate that was not executable.
pub(super) fn direct_not_found_error(command: &str, path: &camino::Utf8Path) -> ResolveError {
    ResolveError::DirectNotFound {
        command: command.to_owned(),
        path: path.to_path_buf(),
    }
}

fn not_found_message(command: &str, dirs: &[Utf8PathBuf], mode: CwdMode) -> String {
    let mut message = localization::message(keys::STDLIB_WHICH_NOT_FOUND)
        .with_arg("command", command)
        .with_arg("count", dirs.len())
        .with_arg("preview", path_preview(dirs))
        .to_string();
    if let Some(hint) = hint_for_mode(mode) {
        message.push_str(". ");
        message.push_str(&hint.to_string());
    }
    message
}

fn direct_not_found_message(command: &str, path: &camino::Utf8Path) -> String {
    localization::message(keys::STDLIB_WHICH_DIRECT_NOT_FOUND)
        .with_arg("command", command)
        .with_arg("path", path.as_str())
        .to_string()
}

fn args_message(detail: impl fmt::Display) -> String {
    localization::message(keys::STDLIB_WHICH_ARGS_ERROR)
        .with_arg("details", detail.to_string())
        .to_string()
}

fn canonicalise_message(path: &camino::Utf8Path, source: &io::Error) -> String {
    localization::message(keys::STDLIB_WHICH_CANONICALISE_FAILED)
        .with_arg("path", path.as_str())
        .with_arg("details", source.to_string())
        .to_string()
}

fn workspace_non_utf8_message(command: &str, path: &str) -> String {
    localization::message(keys::STDLIB_WHICH_WORKSPACE_NON_UTF8)
        .with_arg("command", command)
        .with_arg("path", path)
        .to_string()
}

fn path_preview(dirs: &[Utf8PathBuf]) -> String {
    const LIMIT: usize = 4;
    if dirs.is_empty() {
        return localization::message(keys::STDLIB_WHICH_PATH_PREVIEW_EMPTY).to_string();
    }
    let mut parts: Vec<_> = dirs
        .iter()
        .take(LIMIT)
        .map(|dir| format_path_for_output(dir))
        .collect();
    if dirs.len() > LIMIT {
        parts.push("…".into());
    }
    parts.join(", ")
}

const fn hint_for_mode(mode: CwdMode) -> Option<LocalizedMessage> {
    #[cfg(windows)]
    {
        match mode {
            CwdMode::Always => None,
            _ => Some(localization::message(
                keys::STDLIB_WHICH_NOT_FOUND_HINT_CWD_ALWAYS,
            )),
        }
    }
    #[cfg(not(windows))]
    {
        match mode {
            CwdMode::Never => Some(localization::message(
                keys::STDLIB_WHICH_NOT_FOUND_HINT_CWD_AUTO,
            )),
            _ => None,
        }
    }
}
