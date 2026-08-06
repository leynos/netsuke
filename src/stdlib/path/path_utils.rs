//! Path utilities backing stdlib filters for UTF-8 paths: basename/dirname, `with_suffix`,
//! `relative_to`, canonicalize/realpath, and expanduser with Windows HOME fallbacks. Uses cap-std
//! directory handles and consistent error mapping for template errors.
use std::io;

use cap_std::{ambient_authority, fs_utf8::Dir};

use camino::{Utf8Path, Utf8PathBuf};
use minijinja::{Error, ErrorKind};

use super::fs_utils::{ParentDir, open_parent_dir};
use crate::localization::{self, keys};
use crate::stdlib::config_types::HomeDirectory;
use crate::stdlib::io_helpers::io_to_error;

pub(super) fn basename(path: &Utf8Path) -> String {
    path.file_name().unwrap_or(path.as_str()).to_owned()
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
            localization::message(keys::STDLIB_PATH_WITH_SUFFIX_EMPTY_SEPARATOR).to_string(),
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
        .map(|p| p.as_str().to_owned())
        .map_err(|_| {
            Error::new(
                ErrorKind::InvalidOperation,
                localization::message(keys::STDLIB_PATH_RELATIVE_TO_MISMATCH)
                    .with_arg("path", path.as_str())
                    .with_arg("root", root.as_str())
                    .to_string(),
            )
        })
}

pub(super) fn canonicalize_any(path: &Utf8Path) -> Result<Utf8PathBuf, Error> {
    if path.as_str().is_empty() || path == Utf8Path::new(".") {
        return current_dir_utf8().map_err(|err| {
            io_to_error(
                Utf8Path::new("."),
                &localization::message(keys::STDLIB_PATH_ACTION_CANONICALIZE),
                err,
            )
        });
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
        .map_err(|err| {
            io_to_error(
                path,
                &localization::message(keys::STDLIB_PATH_ACTION_CANONICALIZE),
                err,
            )
        })
}

pub(super) fn is_user_specific_expansion(stripped: &str) -> bool {
    matches!(
        stripped.chars().next(),
        Some(first) if first != '/' && first != std::path::MAIN_SEPARATOR
    )
}

pub(super) fn expanduser<F>(
    raw: &str,
    home_directory: &HomeDirectory,
    read_env: F,
) -> Result<String, Error>
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(stripped) = raw.strip_prefix('~') {
        if is_user_specific_expansion(stripped) {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                localization::message(keys::STDLIB_PATH_EXPANDUSER_UNSUPPORTED).to_string(),
            ));
        }
        let home = resolve_home(home_directory, read_env)?;
        Ok(format!("{home}{stripped}"))
    } else {
        Ok(raw.to_owned())
    }
}

pub(super) fn normalise_parent(parent: Option<&Utf8Path>) -> Utf8PathBuf {
    parent
        .filter(|p| !p.as_str().is_empty())
        .map_or_else(|| Utf8PathBuf::from("."), Utf8Path::to_path_buf)
}

fn resolve_home<F>(home_directory: &HomeDirectory, read_env: F) -> Result<String, Error>
where
    F: Fn(&str) -> Option<String>,
{
    let home = match home_directory {
        HomeDirectory::Ambient => home_from_env(read_env),
        HomeDirectory::Missing => None,
        HomeDirectory::Explicit(home) => Some(home.clone()),
    };
    home.ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::STDLIB_PATH_EXPANDUSER_NO_HOME).to_string(),
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

/// Select the platform ladder and drive it with the injected reader.
///
/// Only the *selection* is platform-gated; neither ladder holds
/// platform-specific logic, which is what keeps both reachable from any host.
/// The reader is injected all the way from the filter-registration boundary,
/// so this module holds no process access of its own: whoever registers the
/// `expanduser` filter decides what [`HomeDirectory::Ambient`] consults.
fn home_from_env<F>(read_env: F) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    #[cfg(windows)]
    {
        windows_home_from(read_env)
    }
    #[cfg(not(windows))]
    {
        posix_home_from(read_env)
    }
}

/// Resolve the home directory using the POSIX precedence ladder.
///
/// Free of platform gating in its *logic*; the `cfg` attribute controls only
/// whether it is compiled, and the platform selection lives in
/// [`home_from_env`].
///
/// # Examples
///
/// ```rust,ignore
/// let env = |key: &str| (key == "HOME").then(|| String::from("/home/a"));
/// assert_eq!(posix_home_from(env).as_deref(), Some("/home/a"));
/// ```
#[cfg(any(not(windows), test))]
pub(super) fn posix_home_from<F>(read_env: F) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    read_env("HOME").or_else(|| read_env("USERPROFILE"))
}

/// Resolve the home directory using the Windows precedence ladder.
///
/// `HOME` and `USERPROFILE` first, then the `HOMEDRIVE`/`HOMEPATH` pair, and
/// finally `HOMESHARE`. An empty `HOMEPATH` is treated as unset, because
/// joining it to `HOMEDRIVE` would yield a bare drive letter rather than a home
/// directory.
///
/// Compiled on Windows and under `test`. Gated to `#[cfg(windows)]` alone this
/// ladder could not be exercised from the Unix CI host, and it is the more
/// intricate of the two; compiled unconditionally it would be dead code in a
/// Unix release build, which `-D warnings` rejects.
///
/// # Examples
///
/// ```rust,ignore
/// // The drive and path pair combine when HOMEPATH is non-empty.
/// let env = |key: &str| match key {
///     "HOMEDRIVE" => Some(String::from("C:")),
///     "HOMEPATH" => Some(String::from("\\me")),
///     _ => None,
/// };
/// assert_eq!(windows_home_from(env).as_deref(), Some("C:\\me"));
/// ```
#[cfg(any(windows, test))]
pub(super) fn windows_home_from<F>(read_env: F) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    read_env("HOME")
        .or_else(|| read_env("USERPROFILE"))
        .or_else(|| match (read_env("HOMEDRIVE"), read_env("HOMEPATH")) {
            // Both halves must be non-empty. An empty HOMEDRIVE would yield a
            // bare `\me`, which names a path on the current drive rather than
            // a home directory; an empty HOMEPATH would yield a bare `C:`,
            // which names a drive. Either way the pair is incomplete, so fall
            // through to HOMESHARE.
            (Some(drive), Some(path)) if !drive.is_empty() && !path.is_empty() => {
                Some(format!("{drive}{path}"))
            }
            _ => read_env("HOMESHARE"),
        })
}
