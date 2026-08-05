//! Snapshot of PATH, PATHEXT, and current directory for the `which` resolver.

use std::ffi::{OsStr, OsString};

use camino::{Utf8Path, Utf8PathBuf};
#[cfg(windows)]
use indexmap::IndexSet;
use mockable::{DefaultEnv, Env};

use crate::localization::{self, keys};

use super::{options::CwdMode, resolve_error::ResolveError};

#[derive(Clone, Debug)]
pub(super) struct EnvSnapshot {
    pub(super) cwd: Utf8PathBuf,
    pub(super) raw_path: Option<OsString>,
    pub(super) raw_pathext: Option<OsString>,
    entries: Vec<PathEntry>,
    #[cfg(windows)]
    pathext: Vec<String>,
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
        #[cfg(windows)]
        return Self::capture_impl(cwd_override, path_override, env, None);
        #[cfg(not(windows))]
        Self::capture_impl(cwd_override, path_override, env)
    }

    #[cfg(windows)]
    pub(super) fn capture_with_pathext(
        cwd_override: Option<&Utf8Path>,
        path_override: Option<&OsStr>,
        pathext_override: Option<&OsStr>,
    ) -> Result<Self, ResolveError> {
        Self::capture_impl(cwd_override, path_override, &DefaultEnv, pathext_override)
    }

    #[cfg(not(windows))]
    fn capture_impl(
        cwd_override: Option<&Utf8Path>,
        path_override: Option<&OsStr>,
        env: &impl Env,
    ) -> Result<Self, ResolveError> {
        let (cwd, raw_path, entries) = capture_common(cwd_override, path_override, env)?;
        Ok(Self {
            cwd,
            raw_path,
            raw_pathext: None,
            entries,
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
        Ok(Self {
            cwd,
            raw_path,
            raw_pathext,
            entries,
            pathext,
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

#[cfg(windows)]
pub(super) const DEFAULT_PATHEXT: &[&str] = &[
    ".com", ".exe", ".bat", ".cmd", ".vbs", ".vbe", ".js", ".jse", ".wsf", ".wsh", ".msc",
];

#[cfg(windows)]
fn parse_pathext(raw: Option<&OsStr>) -> Vec<String> {
    let mut dedup = IndexSet::new();
    let source = raw
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| DEFAULT_PATHEXT.join(";"));
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
        DEFAULT_PATHEXT.iter().map(|ext| ext.to_string()).collect()
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
mod tests {
    //! Unit tests for injected executable-search environment capture.

    use super::*;
    use mockable::MockEnv;

    #[test]
    fn capture_uses_the_injected_path_provider() {
        let cwd = Utf8Path::new("/workspace");
        let configured = OsString::from("/configured/bin");
        let expected = configured.clone();
        let mut env = MockEnv::new();
        env.expect_os_string()
            .withf(|key| key == "PATH")
            .once()
            .return_once(move |_| Some(configured));

        let snapshot = EnvSnapshot::capture_with_env(Some(cwd), None, &env)
            .expect("injected PATH should produce an environment snapshot");

        assert_eq!(snapshot.raw_path, Some(expected));
        assert_eq!(
            snapshot.resolved_dirs(CwdMode::Never),
            [Utf8Path::new("/configured/bin")]
        );
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    //! Windows-specific injected `PATH` and `PATHEXT` capture tests.

    use super::*;
    use mockable::MockEnv;

    #[test]
    fn capture_uses_injected_and_normalized_pathext() {
        let mut env = MockEnv::new();
        env.expect_os_string()
            .withf(|key| key == "PATH")
            .once()
            .return_once(|_| Some(OsString::from(r"C:\configured\bin")));
        env.expect_os_string()
            .withf(|key| key == "PATHEXT")
            .once()
            .return_once(|_| Some(OsString::from(".EXE;exe; CMD ;.cmd")));

        let snapshot =
            EnvSnapshot::capture_with_env(Some(Utf8Path::new("C:/workspace")), None, &env)
                .expect("injected PATH and PATHEXT should produce an environment snapshot");

        assert_eq!(snapshot.pathext(), [".exe", ".cmd"]);
    }
}
