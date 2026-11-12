//! Path comparison helpers and candidate derivation for configuration discovery.

use ortho_config::EnvSource;
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
    if let Some(base) = non_empty(env_source, "XDG_CONFIG_HOME") {
        push_base_candidates(&mut candidates, &base);
    }
    for key in ["APPDATA", "LOCALAPPDATA"] {
        if let Some(base) = non_empty(env_source, key) {
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

/// Read `key` as a base directory, treating an empty value as unset.
///
/// An empty value joined with a file name yields a working-directory relative
/// candidate, which would load configuration from wherever the tool happened to
/// be run rather than from a user location.
fn non_empty(env_source: &dyn EnvSource, key: &str) -> Option<PathBuf> {
    env_source
        .get(key)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Resolve the home directory from the injected environment.
///
/// `HOME` outranks `USERPROFILE`, and an empty value counts as unset so a
/// populated `USERPROFILE` stays reachable. The environment source's own
/// `home_fallback` is deliberately not consulted: Netsuke injects a closed set
/// of documented variables, and a platform lookup would make the candidate list
/// depend on the host rather than on the environment Netsuke was given.
fn resolve_home(env_source: &dyn EnvSource) -> Option<PathBuf> {
    non_empty(env_source, "HOME").or_else(|| non_empty(env_source, "USERPROFILE"))
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
}
