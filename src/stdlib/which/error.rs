//! Error helpers for the `MiniJinja` `which` filter/function.

use std::fmt;

use camino::{Utf8Path, Utf8PathBuf};
use minijinja::{Error, ErrorKind};

use crate::localization::{self, LocalizedMessage, keys};

use super::{format_path_for_output, options::CwdMode};

pub(super) fn not_found_error(command: &str, dirs: &[Utf8PathBuf], mode: CwdMode) -> Error {
    let count = dirs.len();
    let preview = path_preview(dirs);
    let mut message = localization::message(keys::STDLIB_WHICH_NOT_FOUND)
        .with_arg("command", command)
        .with_arg("count", count)
        .with_arg("preview", preview)
        .to_string();
    if let Some(hint) = hint_for_mode(mode) {
        message.push_str(". ");
        message.push_str(&hint.to_string());
    }
    Error::new(ErrorKind::InvalidOperation, message)
}

pub(super) fn direct_not_found(command: &str, path: &Utf8Path) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        localization::message(keys::STDLIB_WHICH_DIRECT_NOT_FOUND)
            .with_arg("command", command)
            .with_arg("path", path.as_str())
            .to_string(),
    )
}

pub(super) fn args_error(message: impl fmt::Display) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        localization::message(keys::STDLIB_WHICH_ARGS_ERROR)
            .with_arg("details", message.to_string())
            .to_string(),
    )
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
