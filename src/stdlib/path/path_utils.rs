//! Path utilities backing stdlib filters for UTF-8 paths: basename/dirname, `with_suffix`,
//! `relative_to`, canonicalize/realpath, and expanduser with Windows HOME fallbacks. Uses cap-std
//! directory handles and consistent error mapping for template errors.
use std::{io, sync::Once};

use cap_std::{ambient_authority, fs_utf8::Dir};

use camino::{Utf8Path, Utf8PathBuf};
use metrics::{counter, describe_counter};
use minijinja::{Error, ErrorKind};

use super::fs_utils::{ParentDir, open_parent_dir};
use crate::localization::{self, keys};
use crate::stdlib::config_types::HomeDirectory;
use crate::stdlib::io_helpers::io_to_error;

/// Return the final path component, or the whole path when it has no file name.
pub(super) fn basename(path: &Utf8Path) -> String {
    path.file_name().unwrap_or(path.as_str()).to_owned()
}

/// Return the parent directory, normalised to `.` when there is none.
pub(super) fn dirname(path: &Utf8Path) -> String {
    normalise_parent(path.parent()).into_string()
}

/// Replace the trailing `sep`-separated segments of the file name with `suffix`.
///
/// # Errors
///
/// Returns an error when the separator is empty.
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

/// Return `path` relative to `root`, when `root` prefixes it.
///
/// # Errors
///
/// Returns an error when `root` does not prefix `path`.
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

/// Resolve `path` to an absolute canonical path, treating `.` as the current directory.
///
/// Roots and the current directory are returned without filesystem access; any
/// other path is canonicalised through its parent directory handle.
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

/// Whether a `~`-remainder names another user rather than the current home.
pub(super) fn is_user_specific_expansion(stripped: &str) -> bool {
    matches!(
        stripped.chars().next(),
        Some(first) if first != '/' && first != std::path::MAIN_SEPARATOR
    )
}

/// Expand a leading `~` to the configured home directory.
///
/// # Errors
///
/// Returns an error for other-user expansions or when no home can be resolved.
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

/// Return the parent path, falling back to `.` when there is none.
pub(super) fn normalise_parent(parent: Option<&Utf8Path>) -> Utf8PathBuf {
    parent
        .filter(|p| !p.as_str().is_empty())
        .map_or_else(|| Utf8PathBuf::from("."), Utf8Path::to_path_buf)
}

/// Resolve the home and report which source supplied it.
///
/// This is the telemetry boundary for home resolution: the ladders stay pure
/// and merely *return* their bounded source label, and this function is the
/// only place that emits an event or increments a counter. Only the label is
/// recorded — never the resolved home, an environment value, or a variable's
/// contents.
fn resolve_home<F>(home_directory: &HomeDirectory, read_env: F) -> Result<String, Error>
where
    F: Fn(&str) -> Option<String>,
{
    describe_home_metrics();
    let resolved = match home_directory {
        HomeDirectory::Ambient => home_from_env(read_env),
        HomeDirectory::Missing => None,
        HomeDirectory::Explicit(home) => Some((home.clone(), HOME_SOURCE_EXPLICIT)),
    };
    let source = resolved
        .as_ref()
        .map_or(HOME_SOURCE_MISSING, |(_, source)| *source);
    let outcome = if resolved.is_some() {
        HOME_OUTCOME_FOUND
    } else {
        HOME_OUTCOME_UNAVAILABLE
    };
    tracing::debug!(
        event = EXPANDUSER_HOME_EVENT,
        source,
        found = resolved.is_some(),
        "resolved the home directory for expanduser",
    );
    // Exactly one increment per resolution, whatever the outcome, so the
    // counter totals resolutions rather than events: the failure path below
    // adds a second *event* but no second sample.
    counter!(
        EXPANDUSER_HOME_TOTAL,
        "outcome" => outcome,
        "source" => source,
    )
    .increment(1);
    resolved.map(|(home, _)| home).ok_or_else(|| {
        tracing::debug!(
            event = EXPANDUSER_HOME_EVENT,
            source,
            outcome = HOME_OUTCOME_UNAVAILABLE,
            "expanduser found no home directory",
        );
        Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::STDLIB_PATH_EXPANDUSER_NO_HOME).to_string(),
        )
    })
}

/// Whether `path` is a filesystem root.
fn is_root(path: &Utf8Path) -> bool {
    path.parent().is_none() && path.file_name().is_none() && !path.as_str().is_empty()
}

/// Resolve the current directory to an absolute, canonical UTF-8 path.
fn current_dir_utf8() -> Result<Utf8PathBuf, io::Error> {
    let dir = Dir::open_ambient_dir(".", ambient_authority())?;
    dir.canonicalize(Utf8Path::new("."))
}

/// The `event` field naming every home-resolution telemetry event.
pub(super) const EXPANDUSER_HOME_EVENT: &str = "stdlib.expanduser.home";

/// Counts home resolutions by bounded `outcome` and `source`.
///
/// Both labels are drawn from the closed sets below, so the series count is
/// fixed by the code rather than by anything the environment supplies.
pub(super) const EXPANDUSER_HOME_TOTAL: &str = "netsuke_stdlib_expanduser_home_total";

/// The bounded `outcome` recorded when a source supplied a home.
pub(super) const HOME_OUTCOME_FOUND: &str = "found";

/// The bounded `outcome` recorded when no source supplied a home.
pub(super) const HOME_OUTCOME_UNAVAILABLE: &str = "home_unavailable";

/// Describe the home-resolution counter once per process.
fn describe_home_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            EXPANDUSER_HOME_TOTAL,
            "Counts expanduser home resolutions labelled by outcome (found or \
             home_unavailable) and by the bounded source that supplied the home."
        );
    });
}

/// `HOME` supplied the home directory.
pub(super) const HOME_SOURCE_HOME: &str = "home";
/// `USERPROFILE` supplied the home directory.
pub(super) const HOME_SOURCE_USERPROFILE: &str = "userprofile";
/// The `HOMEDRIVE`/`HOMEPATH` pair supplied the home directory.
///
/// Gated with [`windows_home_from`], its only reader: the rungs it labels
/// exist solely on that ladder.
#[cfg(any(windows, test))]
pub(super) const HOME_SOURCE_DRIVE_PATH: &str = "drive_path";
/// `HOMESHARE` supplied the home directory.
///
/// Gated with [`windows_home_from`], its only reader.
#[cfg(any(windows, test))]
pub(super) const HOME_SOURCE_HOMESHARE: &str = "homeshare";
/// A configured [`HomeDirectory::Explicit`] value supplied the home directory.
pub(super) const HOME_SOURCE_EXPLICIT: &str = "explicit";
/// No source supplied a home directory.
pub(super) const HOME_SOURCE_MISSING: &str = "missing";

/// A resolved home paired with the bounded label naming what supplied it.
///
/// The label is a `&'static str` drawn from the closed set above, so it can be
/// recorded as telemetry without ever exposing a path or an environment value.
pub(super) type HomeSource = (String, &'static str);

/// Select the platform ladder and drive it with the injected reader.
///
/// This is the module's only platform selection, and it names just the ladder
/// it selects. Each ladder is gated to its own platform plus `test`, so a
/// release build compiles exactly one of them and the other is absent rather
/// than dead — no reference exists solely to keep the compiler quiet.
///
/// The reader is injected all the way from the filter-registration boundary,
/// so this module holds no process access of its own: whoever registers the
/// `expanduser` filter decides what [`HomeDirectory::Ambient`] consults.
fn home_from_env<F>(read_env: F) -> Option<HomeSource>
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
/// Returns the home alongside the bounded label naming the rung that supplied
/// it, so the caller can report the source without inspecting any value.
///
/// Holds no platform-selection logic of its own — that lives solely in
/// [`home_from_env`] — so the `test` arm of the gate below makes it reachable
/// from any host.
///
/// # Examples
///
/// ```rust,ignore
/// let env = |key: &str| (key == "HOME").then(|| String::from("/home/a"));
/// assert_eq!(posix_home_from(env), Some((String::from("/home/a"), "home")));
/// ```
#[cfg(any(not(windows), test))]
pub(super) fn posix_home_from<F>(read_env: F) -> Option<HomeSource>
where
    F: Fn(&str) -> Option<String>,
{
    read_env("HOME")
        .map(|home| (home, HOME_SOURCE_HOME))
        .or_else(|| read_env("USERPROFILE").map(|home| (home, HOME_SOURCE_USERPROFILE)))
}

/// Resolve the home directory using the Windows precedence ladder.
///
/// `HOME` and `USERPROFILE` first, then the `HOMEDRIVE`/`HOMEPATH` pair, and
/// finally `HOMESHARE`. An empty `HOMEPATH` is treated as unset, because
/// joining it to `HOMEDRIVE` would yield a bare drive letter rather than a home
/// directory.
///
/// Returns the home alongside the bounded label naming the rung that supplied
/// it, so the caller can report the source without inspecting any value.
///
/// Holds no platform-selection logic of its own — that lives solely in
/// [`home_from_env`] — so the `test` arm of the gate below makes it reachable
/// from any host. That arm is what lets the Unix CI host exercise this ladder,
/// the more intricate of the two.
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
/// assert_eq!(
///     windows_home_from(env),
///     Some((String::from("C:\\me"), "drive_path")),
/// );
/// ```
#[cfg(any(windows, test))]
pub(super) fn windows_home_from<F>(read_env: F) -> Option<HomeSource>
where
    F: Fn(&str) -> Option<String>,
{
    read_env("HOME")
        .map(|home| (home, HOME_SOURCE_HOME))
        .or_else(|| read_env("USERPROFILE").map(|home| (home, HOME_SOURCE_USERPROFILE)))
        .or_else(|| match (read_env("HOMEDRIVE"), read_env("HOMEPATH")) {
            // Both halves must be non-empty. An empty HOMEDRIVE would yield a
            // bare `\me`, which names a path on the current drive rather than
            // a home directory; an empty HOMEPATH would yield a bare `C:`,
            // which names a drive. Either way the pair is incomplete, so fall
            // through to HOMESHARE.
            (Some(drive), Some(path)) if !drive.is_empty() && !path.is_empty() => {
                Some((format!("{drive}{path}"), HOME_SOURCE_DRIVE_PATH))
            }
            _ => read_env("HOMESHARE").map(|home| (home, HOME_SOURCE_HOMESHARE)),
        })
}
