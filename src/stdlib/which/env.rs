#[cfg(windows)]
use std::ffi::OsStr;
use std::ffi::OsString;

use camino::{Utf8Path, Utf8PathBuf};
#[cfg(windows)]
use indexmap::IndexSet;
use minijinja::{Error, ErrorKind};

use super::{error::args_error, options::CwdMode};

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
    pub(super) fn capture(cwd_override: Option<&Utf8Path>) -> Result<Self, Error> {
        let cwd = if let Some(override_cwd) = cwd_override {
            override_cwd.to_path_buf()
        } else {
            current_dir_utf8()?
        };
        let raw_path = std::env::var_os("PATH");
        let entries = parse_path_entries(raw_path.clone(), &cwd)?;
        #[cfg(windows)]
        let raw_pathext = std::env::var_os("PATHEXT");
        #[cfg(windows)]
        let pathext = parse_pathext(raw_pathext.as_deref());
        #[cfg(not(windows))]
        let raw_pathext = None;
        Ok(Self {
            cwd,
            raw_path,
            raw_pathext,
            entries,
            #[cfg(windows)]
            pathext,
        })
    }

    pub(super) fn resolved_dirs(&self, mode: CwdMode) -> Vec<Utf8PathBuf> {
        let mut dirs = Vec::new();
        if matches!(mode, CwdMode::Always) {
            dirs.push(self.cwd.clone());
        }
        for entry in &self.entries {
            match entry {
                PathEntry::Dir(path) => dirs.push(path.clone()),
                PathEntry::CurrentDir if matches!(mode, CwdMode::Always | CwdMode::Auto) => {
                    dirs.push(self.cwd.clone());
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

#[derive(Clone, Debug)]
enum PathEntry {
    Dir(Utf8PathBuf),
    CurrentDir,
}

fn parse_path_entries(raw: Option<OsString>, cwd: &Utf8Path) -> Result<Vec<PathEntry>, Error> {
    let mut entries = Vec::new();
    let Some(raw_value) = raw else {
        return Ok(entries);
    };
    for (index, component) in std::env::split_paths(&raw_value).enumerate() {
        if component.as_os_str().is_empty() {
            entries.push(PathEntry::CurrentDir);
            continue;
        }
        let utf8 = Utf8PathBuf::from_path_buf(component).map_err(|_| {
            args_error(format!(
                "PATH entry #{index} contains non-UTF-8 characters; Netsuke requires UTF-8 paths",
            ))
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

pub(super) fn current_dir_utf8() -> Result<Utf8PathBuf, Error> {
    let cwd = std::env::current_dir().map_err(|err| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("failed to resolve current directory: {err}"),
        )
    })?;
    Utf8PathBuf::from_path_buf(cwd).map_err(|_| {
        Error::new(
            ErrorKind::InvalidOperation,
            "current directory contains non-UTF-8 components",
        )
    })
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
