//! Path utilities backing stdlib filters for UTF-8 paths: basename/dirname, `with_suffix`,
//! `relative_to`, canonicalise/realpath, and expanduser with Windows HOME fallbacks. Uses cap-std
//! directory handles and consistent error mapping for template errors.
use std::{env, io};

use cap_std::{ambient_authority, fs_utf8::Dir};

use camino::{Utf8Path, Utf8PathBuf};
use minijinja::{Error, ErrorKind};

use super::fs_utils::{ParentDir, open_parent_dir};
use super::io_helpers::io_to_error;

pub(super) fn basename(path: &Utf8Path) -> String {
    path.file_name().unwrap_or(path.as_str()).to_string()
}

pub(super) fn dirname(path: &Utf8Path) -> String {
    normalise_parent(path.parent()).into_string()
}

pub(super) fn with_suffix(
    path: &Utf8Path,
    suffix: &str,
    count: usize,
    sep: &str,
) -> Result<Utf8PathBuf, Error> {
    if sep.is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "with_suffix requires a non-empty separator",
        ));
    }
    let mut base = path.to_path_buf();
    let name = base.file_name().map(str::to_owned).unwrap_or_default();
    if !name.is_empty() {
        base.pop();
    }
    let mut stem = name;
    let mut removed = 0;
    while removed < count {
        if let Some(idx) = stem.rfind(sep) {
            stem.truncate(idx);
            removed += 1;
        } else {
            break;
        }
    }
    stem.push_str(suffix);
    let replacement = Utf8PathBuf::from(stem);
    base.push(&replacement);
    Ok(base)
}

pub(super) fn relative_to(path: &Utf8Path, root: &Utf8Path) -> Result<String, Error> {
    path.strip_prefix(root)
        .map(|p| p.as_str().to_string())
        .map_err(|_| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!("{path} is not relative to {root}"),
            )
        })
}

pub(super) fn canonicalize_any(path: &Utf8Path) -> Result<Utf8PathBuf, Error> {
    if path.as_str().is_empty() || path == Utf8Path::new(".") {
        return current_dir_utf8()
            .map_err(|err| io_to_error(Utf8Path::new("."), "canonicalise", err));
    }
    if is_root(path) {
        return Ok(path.to_path_buf());
    }
    let ParentDir {
        handle,
        entry,
        dir_path,
    } = open_parent_dir(path)?;
    handle
        .canonicalize(Utf8Path::new(&entry))
        .map(|resolved| {
            if resolved.is_absolute() {
                resolved
            } else {
                let mut absolute = dir_path;
                absolute.push(&resolved);
                absolute
            }
        })
        .map_err(|err| io_to_error(path, "canonicalise", err))
}

pub(super) fn is_user_specific_expansion(stripped: &str) -> bool {
    matches!(
        stripped.chars().next(),
        Some(first) if first != '/' && first != std::path::MAIN_SEPARATOR
    )
}

pub(super) fn expanduser(raw: &str) -> Result<String, Error> {
    if let Some(stripped) = raw.strip_prefix('~') {
        if is_user_specific_expansion(stripped) {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                "user-specific ~ expansion is unsupported",
            ));
        }
        let home = resolve_home()?;
        Ok(format!("{home}{stripped}"))
    } else {
        Ok(raw.to_string())
    }
}

pub(super) fn normalise_parent(parent: Option<&Utf8Path>) -> Utf8PathBuf {
    parent
        .filter(|p| !p.as_str().is_empty())
        .map_or_else(|| Utf8PathBuf::from("."), Utf8Path::to_path_buf)
}

fn resolve_home() -> Result<String, Error> {
    home_from_env().ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidOperation,
            "cannot expand ~: no home directory environment variables are set",
        )
    })
}

fn is_root(path: &Utf8Path) -> bool {
    path.parent().is_none() && path.file_name().is_none() && !path.as_str().is_empty()
}

fn current_dir_utf8() -> Result<Utf8PathBuf, io::Error> {
    let dir = Dir::open_ambient_dir(".", ambient_authority())?;
    dir.canonicalize(Utf8Path::new("."))
}

#[cfg(windows)]
fn home_from_env() -> Option<String> {
    env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .ok()
        .or_else(
            || match (env::var("HOMEDRIVE").ok(), env::var("HOMEPATH").ok()) {
                (Some(drive), Some(path)) if !path.is_empty() => Some(format!("{drive}{path}")),
                _ => env::var("HOMESHARE").ok(),
            },
        )
}

#[cfg(not(windows))]
fn home_from_env() -> Option<String> {
    env::var("HOME").or_else(|_| env::var("USERPROFILE")).ok()
}
