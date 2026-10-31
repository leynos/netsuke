//! Workspace-confined resolution for the file-reading filters.
//!
//! A template supplies a path as text, so every reading filter must decide
//! which directory that text is relative to before it opens anything. The
//! process working directory is the wrong answer: it is not the configured
//! workspace, and opening a parent there hands the caller ambient authority
//! that the rest of the stdlib does not grant. [`ReadingRoot`] carries the
//! workspace capability and its optional absolute spelling, and resolves each
//! request to a path relative to that capability. The capability open then
//! enforces the boundary for the parts a lexical check cannot see, such as a
//! link in an intermediate component.
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
    /// accepted only when it lies beneath the configured workspace path. Both
    /// forms are normalized lexically afterwards, and a request whose `..`
    /// components would climb above the root is refused. Lexical normalization
    /// cannot see a link in an intermediate component; that case is caught by
    /// the capability open, which returns the same diagnostic.
    ///
    /// # Errors
    ///
    /// Returns an outside-workspace template error when the request is
    /// absolute without a configured root path, is not beneath that path, or
    /// escapes the root through `..` components.
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
        normalise_within(&relative).ok_or_else(|| outside_workspace_error(requested))
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
        let component = remaining.next()?;
        if !components_match(component, root_component) {
            return None;
        }
    }
    let mut relative = Utf8PathBuf::new();
    for component in remaining {
        match component {
            Utf8Component::CurDir => {}
            Utf8Component::ParentDir => relative.push(".."),
            Utf8Component::Normal(part) => relative.push(part),
            Utf8Component::RootDir | Utf8Component::Prefix(_) => return None,
        }
    }
    Some(relative)
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

/// Normalize `path` lexically, returning `None` if a `..` escapes it.
///
/// The result is always relative and contains no `.` or `..` components, so a
/// later capability open has nothing left to interpret. Keeping the escape
/// decision here rather than relying on the open alone gives the diagnostic
/// the request's own spelling instead of a resolved path.
fn normalise_within(path: &Utf8Path) -> Option<Utf8PathBuf> {
    let mut normalised = Utf8PathBuf::new();
    for component in path.components() {
        match component {
            Utf8Component::CurDir => {}
            Utf8Component::ParentDir => {
                if !normalised.pop() {
                    return None;
                }
            }
            Utf8Component::Normal(part) => normalised.push(part),
            Utf8Component::RootDir | Utf8Component::Prefix(_) => return None,
        }
    }
    Some(normalised)
}

#[cfg(test)]
mod tests;
