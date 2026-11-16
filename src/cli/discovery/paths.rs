//! Path comparison helpers and candidate derivation for configuration discovery.
//!
//! `OrthoConfig` derives its own candidate list, but that order places every
//! system location above the home fallbacks, so a valid system file can
//! outrank the user's own configuration. This module supplies what a
//! Netsuke-owned user pass needs to run first: the ordered user-scope
//! candidates, and the base directories each one resolves beneath.
//!
//! Two rules keep the derivation independent of process state. A base variable
//! that is empty, or that is not absolute, reads as unset, because a relative
//! value would resolve against the working directory or, on Windows, the
//! current drive. And a candidate the loader reports as absent is probed
//! through a directory capability, so a symbolic link whose target is missing
//! is distinguished from a file that is not installed and fails closed.
//!
//! [`SanitisedUserBases`] applies the first rule to the environment view handed
//! to the dependency's own scan, so neither pass admits a candidate that
//! depends on where the tool happened to run.

use cap_std::ambient_authority;
use cap_std::fs::{Dir, Metadata};
use ortho_config::{EnvSource, SharedEnvSource};
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// Application directory name used beneath every configuration base.
const APP_DIR: &str = "netsuke";

/// Nested configuration file name, matching `OrthoConfig`'s default.
const CONFIG_FILE_NAME: &str = "config.toml";

/// Base-level dotfile name, matching `OrthoConfig`'s default.
const DOTFILE_NAME: &str = ".netsuke.toml";

/// Return the expected project file, or `None` without a working directory.
pub(super) fn project_scope_file(directory: Option<&Path>) -> Option<PathBuf> {
    let root = directory
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())?;
    Some(root.join(".netsuke.toml"))
}

/// Collect the user-scope configuration candidates in preference order.
///
/// `OrthoConfig` 0.10.0 interleaves `XDG_CONFIG_DIRS` — a system scope — between
/// the `XDG_CONFIG_HOME` and home user candidates, and its walk stops at the
/// first file that loads. A valid system file therefore hides the user's own
/// configuration, including the environment and fetch restrictions it carries.
/// Netsuke derives this user pass itself: it mirrors the dependency's user
/// subset in the same order and omits every system location.
///
/// The order is `XDG_CONFIG_HOME`, `APPDATA`, `LOCALAPPDATA`, then the home
/// `.config` tree followed by the home dotfile. Each base contributes its
/// nested `netsuke/config.toml` before its `.netsuke.toml`.
///
/// # Examples
///
/// With only `HOME` set, the three home candidates are produced in order:
///
/// ```ignore
/// let mut env = MapEnv::new();
/// env.insert("HOME", "/home/user");
/// assert_eq!(
///     user_scope_candidates(&env),
///     [
///         PathBuf::from("/home/user/.config/netsuke/config.toml"),
///         PathBuf::from("/home/user/.config/.netsuke.toml"),
///         PathBuf::from("/home/user/.netsuke.toml"),
///     ]
/// );
/// ```
pub(super) fn user_scope_candidates(env_source: &dyn EnvSource) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(base) = absolute_base(env_source, "XDG_CONFIG_HOME") {
        push_base_candidates(&mut candidates, &base);
    }
    for key in ["APPDATA", "LOCALAPPDATA"] {
        if let Some(base) = absolute_base(env_source, key) {
            push_base_candidates(&mut candidates, &base);
        }
    }
    if let Some(home) = resolve_home(env_source) {
        push_base_candidates(&mut candidates, &home.join(".config"));
        candidates.push(home.join(DOTFILE_NAME));
    }
    candidates
}

/// Append a base's nested configuration file and dotfile in that order.
fn push_base_candidates(candidates: &mut Vec<PathBuf>, base: &Path) {
    candidates.push(base.join(APP_DIR).join(CONFIG_FILE_NAME));
    candidates.push(base.join(DOTFILE_NAME));
}

/// Read `key` as an absolute base directory, treating anything else as unset.
///
/// An empty or relative value joined with a file name yields a
/// working-directory relative candidate, which would load configuration from
/// wherever the tool happened to be run rather than from a user location. Such a
/// file would then be merged with operator authority, so the value is ignored
/// and the next base is tried, exactly as for an empty value.
///
/// The predicate is `is_absolute()` rather than `has_root()`. On Windows a bare
/// leading separator such as `\config` is rooted but carries no drive, so it
/// resolves on whichever drive the process happens to run from — the same
/// process-state dependence the filter exists to exclude, merely anchored at the
/// current drive instead of the working directory.
fn absolute_base(env_source: &dyn EnvSource, key: &str) -> Option<PathBuf> {
    env_source
        .get(key)
        .filter(|value| is_usable_base(value))
        .map(PathBuf::from)
}

/// Report whether `value` may be used as a configuration base directory.
///
/// This is the single predicate behind both the candidate derivation above and
/// the sanitised view handed to the dependency's fallback, so the two cannot
/// disagree about which bases are usable.
fn is_usable_base(value: &OsStr) -> bool {
    Path::new(value).is_absolute()
}

/// The user base variables whose unusable values must read as unset.
///
/// The list mirrors the bases [`user_scope_candidates`] derives from. The
/// system-scope variable is deliberately absent: its handling stays with the
/// dependency.
const USER_BASE_KEYS: [&str; 5] = [
    "XDG_CONFIG_HOME",
    "APPDATA",
    "LOCALAPPDATA",
    "HOME",
    "USERPROFILE",
];

/// An environment view that hides unusable user base values from the fallback.
///
/// [`user_scope_candidates`] rejects an empty or non-absolute base, but
/// `ortho_config` 0.10.0 filters only empty values. Handing the dependency the
/// raw environment would let its own scan resolve a relative base against the
/// working directory and load configuration from wherever the tool happened to
/// run — the hazard the filter exists to remove. This view applies the same
/// predicate to the dependency's lookups, so a value the user pass rejected
/// cannot come back through the fallback.
pub(super) struct SanitisedUserBases {
    /// The environment whose lookups this view filters.
    source: SharedEnvSource,
}

impl SanitisedUserBases {
    /// Wrap `source` so unusable user base values read as unset.
    pub(super) fn new(source: SharedEnvSource) -> Self {
        Self { source }
    }
}

impl fmt::Debug for SanitisedUserBases {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SanitisedUserBases")
            .field("source", &self.source)
            .finish()
    }
}

impl EnvSource for SanitisedUserBases {
    fn get(&self, key: &str) -> Option<OsString> {
        let value = self.source.get(key)?;
        if USER_BASE_KEYS.contains(&key) && !is_usable_base(&value) {
            return None;
        }
        Some(value)
    }

    fn home_fallback(&self) -> Option<PathBuf> {
        self.source.home_fallback()
    }

    fn config_dir_fallback(&self) -> Option<PathBuf> {
        self.source.config_dir_fallback()
    }
}

/// Resolve the home directory from the injected environment.
///
/// `HOME` outranks `USERPROFILE`, and an empty, relative, or drive-relative
/// value counts as unset so a populated `USERPROFILE` stays reachable. The environment source's own
/// `home_fallback` is deliberately not consulted: Netsuke injects a closed set
/// of documented variables, and a platform lookup would make the candidate list
/// depend on the host rather than on the environment Netsuke was given.
fn resolve_home(env_source: &dyn EnvSource) -> Option<PathBuf> {
    absolute_base(env_source, "HOME").or_else(|| absolute_base(env_source, "USERPROFILE"))
}

/// Compare a canonical project path, falling back literally if resolution fails.
///
/// Missing optional files must not fail discovery. The project loading pass
/// can still compare canonical chain roots if normalization is unavailable.
pub(super) fn comparison_key(normalizer: &impl PathNormalizer, path: &str) -> PathBuf {
    normalized_path_key(normalizer, path).unwrap_or_else(|_| PathBuf::from(path))
}

/// Resolves a path to the canonical form used for layer comparison.
///
/// This is the seam through which discovery reaches the filesystem, so tests can
/// force a resolution failure without depending on the ambient environment.
pub(super) trait PathNormalizer {
    /// Resolve `path`, propagating any I/O error to the caller.
    fn normalize(&self, path: &Path) -> io::Result<PathBuf>;

    /// Report whether `path` names an existing filesystem entry.
    ///
    /// Callers use this only to decide what an *unresolvable* path means, so the
    /// query stays on this seam rather than reaching for `Path::exists`. That
    /// inherent method collapses every I/O error into `false`, which would let
    /// an unreadable directory entry masquerade as an absent file; reporting the
    /// error instead keeps that distinction available to the caller.
    fn exists(&self, path: &Path) -> io::Result<bool>;
}

/// Production normalizer that exactly mirrors `OrthoConfig`'s canonical paths.
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct FsPathNormalizer;

impl PathNormalizer for FsPathNormalizer {
    fn normalize(&self, path: &Path) -> io::Result<PathBuf> {
        // `ortho_config` canonicalizes layer paths with `dunce` on Windows so
        // diagnostics and comparisons stay free of UNC prefixes and short-name
        // forms; mirror that here so the project-scope dedup key and the
        // injected explicit config path compare equal to the recorded layer
        // path. On other platforms `dunce` is a thin wrapper over
        // `std::fs::canonicalize`, so the behaviour is unchanged.
        dunce::canonicalize(path)
    }

    fn exists(&self, path: &Path) -> io::Result<bool> {
        path.try_exists()
    }
}

/// Return the canonical comparison key for `path`.
///
/// `OrthoConfig` canonicalizes every layer path it records, whereas the expected
/// project path is joined from the caller's `--directory` verbatim. Normalizing
/// the expected project path keeps a relative or symlinked directory from
/// looking like a different file without re-reading each stored layer path.
///
/// Resolution failure is reported rather than absorbed: the caller decides what
/// an unresolvable path means. See `collect_file_layers`, which compares such a
/// path literally so a missing project `.netsuke.toml` or an unreadable
/// directory does not fail configuration discovery.
pub(super) fn normalized_path_key(
    normalizer: &impl PathNormalizer,
    path: &str,
) -> io::Result<PathBuf> {
    normalizer.normalize(Path::new(path))
}

/// Inspect a candidate entry without following a link at its final component.
///
/// The probe holds only the candidate's parent directory, so it reads no more
/// authority than the discovery loader that reports an unresolvable candidate as
/// absent. A missing entry, or a missing parent directory, reports absence.
///
/// # Errors
///
/// Returns an I/O error when the parent directory cannot be opened or the entry
/// cannot be inspected.
pub(super) fn candidate_entry_metadata(path: &Path) -> io::Result<Option<Metadata>> {
    let Some(name) = path.file_name() else {
        return Ok(None);
    };
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let dir = match Dir::open_ambient_dir(parent, ambient_authority()) {
        Ok(dir) => dir,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    match dir.symlink_metadata(name) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// A normalizer that always fails, so the failure branch is deterministic.
///
/// Real canonicalization failure depends on the ambient filesystem — a missing
/// file, an unreadable directory — which a test cannot force portably.
#[cfg(test)]
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct FailingPathNormalizer;

#[cfg(test)]
impl PathNormalizer for FailingPathNormalizer {
    fn normalize(&self, _path: &Path) -> io::Result<PathBuf> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "normalization refused for test",
        ))
    }

    /// Defer existence to the real filesystem, which the fixture controls.
    ///
    /// This double models a path that will not *resolve*; the cases that use it
    /// distinguish a missing project file from an existing one, so the
    /// existence answer must still be genuine.
    fn exists(&self, path: &Path) -> io::Result<bool> {
        FsPathNormalizer.exists(path)
    }
}

/// A normalizer that cannot resolve a path or report whether it exists.
///
/// An unreadable directory entry is not the same fact as a missing file, and
/// discovery must be able to tell the two apart. The real filesystem produces
/// this state for the agent's own tests only if permissions are arranged just
/// so, which a test cannot do portably.
#[cfg(test)]
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct UnreadablePathNormalizer;

#[cfg(test)]
impl PathNormalizer for UnreadablePathNormalizer {
    fn normalize(&self, _path: &Path) -> io::Result<PathBuf> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "normalization refused for test",
        ))
    }

    fn exists(&self, _path: &Path) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "existence query refused for test",
        ))
    }
}
