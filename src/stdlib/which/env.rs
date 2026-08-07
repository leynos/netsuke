//! Snapshot of PATH, PATHEXT, and current directory for the `which` resolver.

use std::ffi::{OsStr, OsString};

use camino::{Utf8Path, Utf8PathBuf};
#[cfg(any(windows, test))]
use indexmap::IndexSet;
use mockable::{DefaultEnv, Env};

use crate::localization::{self, keys};

use super::{
    options::CwdMode,
    resolve_error::ResolveError,
    workspace_switch::{WORKSPACE_FALLBACK_ENV, WorkspaceSwitch},
};

/// Translate a platform reading of the switch into its domain state.
///
/// The conversion lives here, in the adapter that performs the read, so
/// `workspace_switch` never names `std::env::VarError`. It is the single
/// place the platform error crosses into the resolver's own vocabulary.
impl From<Result<String, std::env::VarError>> for WorkspaceSwitch {
    fn from(raw: Result<String, std::env::VarError>) -> Self {
        match raw {
            Ok(value) => Self::Value(value),
            Err(std::env::VarError::NotPresent) => Self::Absent,
            Err(std::env::VarError::NotUnicode(_)) => Self::NotUnicode,
        }
    }
}

/// Read and translate the workspace switch, warning about a mis-encoded value.
///
/// Both capture variants funnel through here so the diagnostic fires exactly
/// once per capture, at the boundary where the ambient read happens, rather
/// than on every consultation of the switch. Silently switching workspace
/// search off would leave a user whose variable is mis-encoded with commands
/// mysteriously unresolved and no indication why. Only the variable's name is
/// logged; its value never is.
fn capture_workspace_switch(env: &impl Env) -> WorkspaceSwitch {
    let switch = WorkspaceSwitch::from(env.raw(WORKSPACE_FALLBACK_ENV));
    if matches!(switch, WorkspaceSwitch::NotUnicode) {
        tracing::warn!(
            env = WORKSPACE_FALLBACK_ENV,
            "workspace fallback disabled because env var is not valid UTF-8",
        );
    }
    switch
}

#[derive(Clone, Debug)]
pub(super) struct EnvSnapshot {
    pub(super) cwd: Utf8PathBuf,
    pub(super) raw_path: Option<OsString>,
    pub(super) raw_pathext: Option<OsString>,
    entries: Vec<PathEntry>,
    #[cfg(windows)]
    pathext: Vec<String>,
    /// The `NETSUKE_WHICH_WORKSPACE` state captured for this snapshot.
    ///
    /// Stored as data rather than a decision so the cache fingerprint can
    /// hash it — two resolutions differing only in this switch must not
    /// share a cache entry — and so `env` depends only on the leaf
    /// `workspace_switch` module rather than calling into `lookup`.
    workspace_switch: WorkspaceSwitch,
}

impl EnvSnapshot {
    pub(super) fn capture(
        cwd_override: Option<&Utf8Path>,
        path_override: Option<&OsStr>,
    ) -> Result<Self, ResolveError> {
        Self::capture_with_env(cwd_override, path_override, &DefaultEnv)
    }

    pub(super) fn capture_with_env(
        cwd_override: Option<&Utf8Path>,
        path_override: Option<&OsStr>,
        env: &impl Env,
    ) -> Result<Self, ResolveError> {
        Self::capture_for_platform(cwd_override, path_override, env)
    }

    /// Capture a snapshot without a `PATHEXT` override on Windows.
    ///
    /// Windows threads a `PATHEXT` override that the other platforms have no
    /// concept of, so the two `capture_impl` arities diverge. Isolating the
    /// divergence in a pair of wrappers keeps `capture_with_env` free of a
    /// `cfg`-gated bare `return`, which reads as dead code on either target.
    #[cfg(windows)]
    fn capture_for_platform(
        cwd_override: Option<&Utf8Path>,
        path_override: Option<&OsStr>,
        env: &impl Env,
    ) -> Result<Self, ResolveError> {
        Self::capture_impl(cwd_override, path_override, env, None)
    }

    /// Capture a snapshot on platforms without `PATHEXT` semantics.
    ///
    /// See the Windows counterpart for why this wrapper exists.
    #[cfg(not(windows))]
    fn capture_for_platform(
        cwd_override: Option<&Utf8Path>,
        path_override: Option<&OsStr>,
        env: &impl Env,
    ) -> Result<Self, ResolveError> {
        Self::capture_impl(cwd_override, path_override, env)
    }

    /// Capture with an explicit `PATHEXT`, shadowing the process value.
    ///
    /// Defined on every platform so the resolver has one capture entry point:
    /// the caller need not fork on the target to pass an override through.
    #[cfg(windows)]
    pub(super) fn capture_with_pathext(
        cwd_override: Option<&Utf8Path>,
        path_override: Option<&OsStr>,
        pathext_override: Option<&OsStr>,
    ) -> Result<Self, ResolveError> {
        Self::capture_impl(cwd_override, path_override, &DefaultEnv, pathext_override)
    }

    /// Capture ignoring the supplied `PATHEXT`.
    ///
    /// `PATHEXT` has no meaning outside Windows — nothing consults the
    /// snapshot's extension list there — so the override is accepted and
    /// discarded rather than forcing every caller to gate on the target.
    #[cfg(not(windows))]
    pub(super) fn capture_with_pathext(
        cwd_override: Option<&Utf8Path>,
        path_override: Option<&OsStr>,
        _pathext_override: Option<&OsStr>,
    ) -> Result<Self, ResolveError> {
        Self::capture(cwd_override, path_override)
    }

    #[cfg(not(windows))]
    fn capture_impl(
        cwd_override: Option<&Utf8Path>,
        path_override: Option<&OsStr>,
        env: &impl Env,
    ) -> Result<Self, ResolveError> {
        let (cwd, raw_path, entries) = capture_common(cwd_override, path_override, env)?;
        let workspace_switch = capture_workspace_switch(env);
        Ok(Self {
            cwd,
            raw_path,
            raw_pathext: None,
            entries,
            workspace_switch,
        })
    }

    #[cfg(windows)]
    fn capture_impl(
        cwd_override: Option<&Utf8Path>,
        path_override: Option<&OsStr>,
        env: &impl Env,
        pathext_override: Option<&OsStr>,
    ) -> Result<Self, ResolveError> {
        let (cwd, raw_path, entries) = capture_common(cwd_override, path_override, env)?;
        let raw_pathext = pathext_override
            .map(OsString::from)
            .or_else(|| env.os_string("PATHEXT"));
        let pathext = parse_pathext(raw_pathext.as_deref());
        let workspace_switch = capture_workspace_switch(env);
        Ok(Self {
            cwd,
            raw_path,
            raw_pathext,
            entries,
            pathext,
            workspace_switch,
        })
    }

    /// List the directories to search, borrowing them from the snapshot.
    ///
    /// Returns references rather than owned paths: the search loop and the
    /// miss diagnostics only read the directories, and the error path copies
    /// them into the owned [`super::resolve_error::ResolveError`] at the
    /// boundary where the data outlives the snapshot.
    pub(super) fn resolved_dirs(&self, mode: CwdMode) -> Vec<&Utf8Path> {
        let mut dirs = Vec::new();
        let mut cwd_added = matches!(mode, CwdMode::Always);
        if cwd_added {
            dirs.push(self.cwd.as_path());
        }
        for entry in &self.entries {
            match entry {
                PathEntry::Dir(path) => dirs.push(path.as_path()),
                // The working directory is searched at most once: `Always`
                // has already prepended it, and repeated current-directory
                // PATH entries (for example `::/usr/bin::`) collapse to the
                // first occurrence.
                PathEntry::CurrentDir if matches!(mode, CwdMode::Auto) && !cwd_added => {
                    cwd_added = true;
                    dirs.push(self.cwd.as_path());
                }
                PathEntry::CurrentDir => {}
            }
        }
        dirs
    }

    #[cfg(windows)]
    pub(super) fn pathext(&self) -> &[String] {
        &self.pathext
    }

    /// Whether the workspace fallback is enabled for this snapshot.
    ///
    /// Decided on demand from the captured state; the non-UTF-8 warning has
    /// already fired once at capture, so repeated consultations of the switch
    /// stay silent.
    pub(super) fn workspace_fallback_enabled(&self) -> bool {
        self.workspace_switch.enabled()
    }

    /// Replace the captured switch state, for cache-key tests.
    #[cfg(test)]
    pub(super) fn with_workspace_switch(mut self, switch: WorkspaceSwitch) -> Self {
        self.workspace_switch = switch;
        self
    }

    /// The captured switch state, as the cache fingerprint hashes it.
    pub(super) const fn workspace_switch(&self) -> &WorkspaceSwitch {
        &self.workspace_switch
    }
}

fn capture_common(
    cwd_override: Option<&Utf8Path>,
    path_override: Option<&OsStr>,
    env: &impl Env,
) -> Result<(Utf8PathBuf, Option<OsString>, Vec<PathEntry>), ResolveError> {
    let cwd = if let Some(override_cwd) = cwd_override {
        override_cwd.to_path_buf()
    } else {
        current_dir_utf8()?
    };
    let raw_path = path_override
        .map(OsString::from)
        .or_else(|| env.os_string("PATH"));
    let entries = parse_path_entries(raw_path.as_deref(), &cwd)?;
    Ok((cwd, raw_path, entries))
}

#[derive(Clone, Debug)]
enum PathEntry {
    Dir(Utf8PathBuf),
    CurrentDir,
}

fn parse_path_entries(raw: Option<&OsStr>, cwd: &Utf8Path) -> Result<Vec<PathEntry>, ResolveError> {
    let mut entries = Vec::new();
    let Some(raw_value) = raw else {
        return Ok(entries);
    };
    for (index, component) in std::env::split_paths(raw_value).enumerate() {
        if component.as_os_str().is_empty() {
            entries.push(PathEntry::CurrentDir);
            continue;
        }
        let utf8 = Utf8PathBuf::from_path_buf(component).map_err(|_| {
            ResolveError::args(
                localization::message(keys::STDLIB_WHICH_PATH_ENTRY_NON_UTF8)
                    .with_arg("index", index),
            )
        })?;
        let resolved = if utf8.is_absolute() {
            utf8
        } else {
            cwd.join(utf8)
        };
        entries.push(PathEntry::Dir(resolved));
    }
    Ok(entries)
}

/// Extensions Windows treats as executable when `PATHEXT` is unset or empty.
///
/// Compiled on Windows and under `test`, alongside [`parse_pathext`], which
/// falls back to it.
#[cfg(any(windows, test))]
pub(super) const DEFAULT_PATHEXT: &[&str] = &[
    ".com", ".exe", ".bat", ".cmd", ".vbs", ".vbe", ".js", ".jse", ".wsf", ".wsh", ".msc",
];

/// Own the built-in list so the fallback has a single construction site.
///
/// The entries are already lowercase and dot-prefixed, so they need no further
/// normalization.
#[cfg(any(windows, test))]
fn default_pathext() -> Vec<String> {
    DEFAULT_PATHEXT.iter().copied().map(String::from).collect()
}

/// Normalize a raw `PATHEXT` value into lowercase, dot-prefixed extensions.
///
/// Pure string handling, consulted only by the Windows snapshot but compiled
/// on Windows *and* under `test`. Gated to `#[cfg(windows)]` alone, its
/// normalization — lowercasing, inserting missing leading dots, trimming,
/// de-duplicating, and falling back to the built-in list when the value yields
/// nothing — could not be exercised from the Unix CI host at all, so every rule
/// it implements went unverified on the platform where the suite actually runs.
/// Compiled unconditionally it would instead be dead code in a Unix release
/// build, which `-D warnings` rejects.
///
/// # Examples
///
/// ```rust,ignore
/// // Values are lowercased, given a leading dot, and de-duplicated.
/// assert_eq!(parse_pathext(Some(OsStr::new("COM;.com"))), vec![".com"]);
/// ```
#[cfg(any(windows, test))]
pub(super) fn parse_pathext(raw: Option<&OsStr>) -> Vec<String> {
    let mut dedup = IndexSet::new();
    let source = raw.map_or_else(
        || DEFAULT_PATHEXT.join(";"),
        |value| value.to_string_lossy().into_owned(),
    );
    for segment in source.split(';') {
        let trimmed = segment.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut normalised = trimmed.to_ascii_lowercase();
        if !normalised.starts_with('.') {
            normalised.insert(0, '.');
        }
        dedup.insert(normalised);
    }
    if dedup.is_empty() {
        default_pathext()
    } else {
        dedup.into_iter().collect()
    }
}

pub(super) fn current_dir_utf8() -> Result<Utf8PathBuf, ResolveError> {
    let cwd = std::env::current_dir().map_err(|source| ResolveError::CwdResolve { source })?;
    Utf8PathBuf::from_path_buf(cwd).map_err(|_| ResolveError::CwdNonUtf8)
}

#[cfg(windows)]
pub(super) fn candidate_paths(
    dir: &Utf8Path,
    command: &str,
    pathext: &[String],
) -> Vec<Utf8PathBuf> {
    let mut paths = Vec::new();
    let base = dir.join(command);
    if Utf8Path::new(command).extension().is_some() {
        paths.push(base);
        return paths;
    }
    for ext in pathext {
        let mut candidate = base.as_str().to_owned();
        candidate.push_str(ext);
        paths.push(Utf8PathBuf::from(candidate));
    }
    paths
}

#[cfg(all(test, not(windows)))]
#[path = "env_tests.rs"]
mod tests;

#[cfg(all(test, windows))]
#[path = "env_windows_tests.rs"]
mod windows_tests;
