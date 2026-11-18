//! Workspace-confined resolution for the file-reading filters.
//!
//! A template supplies a path as text, so every reading filter must decide
//! which directory that text is relative to before it opens anything. The
//! process working directory is the wrong answer: it is not the configured
//! workspace, and opening a parent there hands the caller ambient authority
//! that the rest of the stdlib does not grant. [`ReadingRoot`] carries the
//! workspace capability and its optional absolute spelling, and resolves each
//! request to a path relative to that capability. The request keeps its `..`
//! components, so the filesystem resolves them exactly as it would for a path
//! typed at a shell; only the capability open decides where that leads, and
//! it enforces the boundary for the parts a lexical check cannot see, such as
//! a link in an intermediate component.
use std::io;
use std::sync::Arc;

use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
#[cfg(test)]
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use minijinja::{Error, ErrorKind};

use crate::localization::{self, keys};

/// A workspace capability that file-reading filters resolve requests through.
///
/// `handle` is always the capability the requested file is opened through.
/// `path` is the absolute spelling of the same directory when the caller
/// supplied one; it is only used to recognize an absolute request, never to
/// open anything.
#[derive(Clone)]
pub(crate) struct ReadingRoot {
    /// The capability every requested file is opened through.
    handle: Arc<Dir>,
    /// The handle's absolute spelling, when the caller recorded one.
    path: Option<Utf8PathBuf>,
}

impl ReadingRoot {
    /// Pair a workspace capability with its optional absolute spelling.
    pub(crate) const fn new(handle: Arc<Dir>, path: Option<Utf8PathBuf>) -> Self {
        Self { handle, path }
    }

    /// Resolve a template-supplied path to one relative to the workspace.
    ///
    /// A relative request is used as written, and an absolute request is
    /// accepted only when it lies beneath the configured workspace path, with
    /// that path's spelling removed. `.` components are dropped; every other
    /// component is kept, including `..`, so the filesystem gives the request
    /// the meaning it would give a path typed at a shell: `alias/../file`
    /// resolves against whatever `alias` names, and a request through a
    /// component that does not exist fails when it is opened rather than
    /// being folded away first.
    ///
    /// Only a request that begins by climbing above the root is refused here,
    /// because the workspace capability has no parent to climb into. The
    /// capability open refuses every other way out, such as a link in an
    /// intermediate component, and reports it with the same diagnostic.
    ///
    /// # Errors
    ///
    /// Returns an outside-workspace template error when the request is
    /// absolute without a configured root path, is not beneath that path, or
    /// begins with a `..` component.
    pub(crate) fn resolve(&self, requested: &Utf8Path) -> Result<Utf8PathBuf, Error> {
        let relative = if requested.is_absolute() {
            let root = self
                .path
                .as_deref()
                .ok_or_else(|| outside_workspace_error(requested))?;
            strip_workspace_prefix(requested, root)
                .ok_or_else(|| outside_workspace_error(requested))?
        } else {
            requested.to_path_buf()
        };
        relative_to_root(&relative).ok_or_else(|| outside_workspace_error(requested))
    }

    /// Open the parent directory of an already-resolved relative path.
    ///
    /// The handle is returned rather than the file so the caller keeps the
    /// open policy — the platform flags, the symlink decision, and the
    /// regular-file check — in one place. An empty parent names the workspace
    /// root itself, which is cloned rather than reopened by path.
    ///
    /// # Errors
    ///
    /// Returns the capability's I/O error when the parent cannot be opened.
    /// For a path that escapes the capability this is the escape error that
    /// [`is_capability_escape`] recognizes.
    pub(super) fn open_parent(&self, relative: &Utf8Path) -> io::Result<Dir> {
        relative
            .parent()
            .filter(|parent| !parent.as_str().is_empty())
            .map_or_else(
                || self.handle.try_clone(),
                |parent| self.handle.open_dir(parent),
            )
    }
}

#[cfg(test)]
impl ReadingRoot {
    /// Bind a reading root to the directory holding a test fixture.
    ///
    /// The fixture's parent is the notional workspace the path unit tests read
    /// within, so it supplies both the capability and the absolute spelling a
    /// request resolves against.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixture path has no parent directory, or when
    /// that directory cannot be opened as a capability.
    pub(crate) fn for_fixture(file: &Utf8Path) -> io::Result<Self> {
        let directory = file.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "fixture file has no parent directory",
            )
        })?;
        let handle = Dir::open_ambient_dir(directory, ambient_authority())?;
        Ok(Self::new(Arc::new(handle), Some(directory.to_path_buf())))
    }
}

/// Whether `err` is the capability refusing a path that leaves it.
///
/// cap-std reports an escape as `PermissionDenied` with no operating-system
/// code and its own message, keeping it distinguishable from an ACL denial,
/// which carries the platform's code. Callers use this to report the same
/// workspace diagnostic for a link escape as for a lexical one instead of
/// leaking the platform wording.
pub(crate) fn is_capability_escape(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::PermissionDenied && err.raw_os_error().is_none()
}

/// Build the localized outside-workspace diagnostic for `path`.
pub(crate) fn outside_workspace_error(path: &Utf8Path) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        localization::message(keys::STDLIB_PATH_OUTSIDE_WORKSPACE)
            .with_arg("path", path.as_str())
            .to_string(),
    )
}

/// Strip `root` from the front of `requested`, returning the remainder.
///
/// Components are compared one at a time so a sibling whose name merely
/// shares a prefix — `/tmp/work` and `/tmp/workspace` — cannot be mistaken for
/// a child. Windows compares names case-insensitively, because the same
/// directory is commonly spelled with either case. A verbatim `\\?\` request
/// carries a different prefix component from an ordinary root, so it is
/// refused rather than interpreted.
fn strip_workspace_prefix(requested: &Utf8Path, root: &Utf8Path) -> Option<Utf8PathBuf> {
    let mut remaining = requested.components();
    for root_component in root.components() {
        if !components_match(remaining.next()?, root_component) {
            return None;
        }
    }
    Some(remaining.as_path().to_path_buf())
}

/// Compare two path components under the platform's name rules.
fn components_match(left: Utf8Component<'_>, right: Utf8Component<'_>) -> bool {
    #[cfg(windows)]
    {
        left.as_str().eq_ignore_ascii_case(right.as_str())
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

/// Rewrite `path` relative to the root, or refuse it when it starts above.
///
/// `.` components are dropped because they name nothing. `..` is kept so the
/// capability open interprets it with the filesystem's own rules, which is
/// what lets `alias/../file` mean whatever the directory `alias` names makes
/// it mean. A path whose first meaningful component is `..` has no root-side
/// directory to come back to, so it is refused here rather than left for the
/// open, which would have no name to report.
fn relative_to_root(path: &Utf8Path) -> Option<Utf8PathBuf> {
    let mut relative = Utf8PathBuf::new();
    for component in path.components() {
        match component {
            Utf8Component::CurDir => {}
            Utf8Component::ParentDir if relative.as_str().is_empty() => return None,
            Utf8Component::ParentDir => relative.push(".."),
            Utf8Component::Normal(part) => relative.push(part),
            Utf8Component::RootDir | Utf8Component::Prefix(_) => return None,
        }
    }
    Some(relative)
}

#[cfg(test)]
mod tests;
