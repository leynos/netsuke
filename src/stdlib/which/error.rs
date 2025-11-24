//! Error helpers for the `MiniJinja` `which` filter/function.

use std::fmt;

use camino::{Utf8Path, Utf8PathBuf};
use minijinja::{Error, ErrorKind};

use super::{format_path_for_output, options::CwdMode};

pub(super) fn not_found_error(command: &str, dirs: &[Utf8PathBuf], mode: CwdMode) -> Error {
    let count = dirs.len();
    let preview = path_preview(dirs);
    let mut message = format!(
        "[netsuke::jinja::which::not_found] command '{command}' not found after checking {count} PATH entries. Preview: {preview}",
    );
    if let Some(hint) = hint_for_mode(mode) {
        message.push_str(". ");
        message.push_str(hint);
    }
    Error::new(ErrorKind::InvalidOperation, message)
}

pub(super) fn direct_not_found(command: &str, path: &Utf8Path) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!(
            "[netsuke::jinja::which::not_found] command '{command}' at '{path}' is missing or not executable",
        ),
    )
}

pub(super) fn args_error(message: impl fmt::Display) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!("[netsuke::jinja::which::args] {message}"),
    )
}

fn path_preview(dirs: &[Utf8PathBuf]) -> String {
    const LIMIT: usize = 4;
    if dirs.is_empty() {
        return "<empty>".to_owned();
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

const fn hint_for_mode(mode: CwdMode) -> Option<&'static str> {
    #[cfg(windows)]
    {
        match mode {
            CwdMode::Always => None,
            _ => Some("Set cwd_mode=\"always\" to include the current directory."),
        }
    }
    #[cfg(not(windows))]
    {
        match mode {
            CwdMode::Never => Some(
                "Empty PATH segments are ignored; use cwd_mode=\"auto\" to include the working directory.",
            ),
            _ => None,
        }
    }
}
