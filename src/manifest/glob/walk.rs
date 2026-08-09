//! Filesystem traversal helpers for glob expansion.
//!
//! Glob matching itself is performed by the `glob` crate, which walks the
//! filesystem ambiently. The metadata checks used to filter directories out
//! of the results, however, go through a capability-scoped
//! [`cap_std::fs::Dir`] handle. To honour least privilege, that handle is
//! opened at the pattern's longest literal directory prefix (for example
//! `src/` for `src/**/*.c`) rather than at the filesystem root, so the
//! capability covers only the subtree the pattern can actually match. A
//! symbolic link whose target escapes that subtree is therefore unreadable
//! through the capability; such a match is skipped rather than failing the
//! expansion.
//!
//! The prefix is pattern text, so it is unescaped before it meets the
//! filesystem: a directory the pattern names as `[*]x` is the directory `*x`
//! on disk. Matches keep whatever rooting the pattern had — absolute for an
//! absolute pattern, `../…` for a parent-relative one — so each is rebased
//! onto the prefix before its metadata lookup.

use super::{
    GlobEntryResult, GlobErrorContext, GlobErrorType, GlobPattern, create_glob_error, diagnostics,
};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs::Dir};
use minijinja::Error;
use std::io;

/// Capability root for a glob expansion.
///
/// Couples the [`Dir`] handle opened at the pattern's literal prefix with
/// that prefix, so matched paths can be relativised before metadata lookups.
pub(super) struct GlobRoot {
    dir: Dir,
    prefix: Utf8PathBuf,
}

impl GlobRoot {
    #[cfg(test)]
    pub(super) const fn new(dir: Dir, prefix: Utf8PathBuf) -> Self {
        Self { dir, prefix }
    }

    /// Directory the capability is scoped to.
    #[cfg(test)]
    pub(super) const fn dir(&self) -> &Dir {
        &self.dir
    }

    /// Literal pattern prefix the capability was opened at.
    #[cfg(test)]
    pub(super) fn prefix(&self) -> &Utf8Path {
        self.prefix.as_path()
    }

    /// Fetch metadata for a matched path via the capability-scoped handle.
    ///
    /// Returns `Ok(None)` only when the match is unresolvable in one of the two
    /// ways a symbolic link makes it so: the link escapes the literal prefix,
    /// which `cap_std` reports as [`io::ErrorKind::PermissionDenied`], or it
    /// dangles, which surfaces as [`io::ErrorKind::NotFound`]. The link may be
    /// the final component or an intermediate directory. Either way the match
    /// names no file reachable within the capability, so it is skipped rather
    /// than aborting the whole expansion.
    ///
    /// Every other failure propagates, including a symlink loop
    /// ([`io::ErrorKind::FilesystemLoop`]): a cyclic link is a broken tree
    /// rather than an absent file, and silently dropping it would hide the
    /// breakage. A link whose target is genuinely unreadable inside the prefix
    /// is skipped along with the escapes, because `cap_std` reports both as
    /// `PermissionDenied` and the capability cannot tell them apart.
    pub(super) fn metadata(&self, path: &Utf8Path) -> io::Result<Option<cap_std::fs::Metadata>> {
        let relative = self.relativise(path)?;
        match self.dir.metadata(relative) {
            Ok(metadata) => Ok(Some(metadata)),
            Err(err) if is_unresolvable_link(&err) && self.traverses_symlink(relative) => {
                diagnostics::record_unreachable_symlink(relative);
                Ok(None)
            }
            Err(err) => Err(err),
        }
    }

    /// Report whether any component of `relative` is a symbolic link.
    ///
    /// `symlink_metadata` does not follow its final component, so the match
    /// itself can be settled in one lookup. That is the common case — a link
    /// named directly by the pattern — and it is tried first so the ancestor
    /// walk, which costs one lookup per directory, is only paid for when an
    /// intermediate component is the culprit.
    fn traverses_symlink(&self, relative: &Utf8Path) -> bool {
        if self
            .dir
            .symlink_metadata(relative)
            .is_ok_and(|link| link.is_symlink())
        {
            return true;
        }
        self.ancestor_is_symlink(relative)
    }

    /// Report whether a directory on the way to `relative` is a symbolic link.
    ///
    /// Walks from the capability root outwards, inspecting each ancestor
    /// without following it. The final component is skipped because
    /// [`Self::traverses_symlink`] has already settled it.
    fn ancestor_is_symlink(&self, relative: &Utf8Path) -> bool {
        let mut ancestor = Utf8PathBuf::new();
        let mut components = relative.components().peekable();
        while let Some(component) = components.next() {
            if components.peek().is_none() {
                break;
            }
            ancestor.push(component);
            match self.dir.symlink_metadata(&ancestor) {
                Ok(metadata) if metadata.is_symlink() => return true,
                Ok(_) => {}
                // This ancestor cannot be inspected at all, so no link has
                // been found and the caller's error stands.
                Err(_) => return false,
            }
        }
        false
    }

    /// Rebase a matched path onto the capability prefix.
    pub(super) fn relativise<'a>(&self, path: &'a Utf8Path) -> io::Result<&'a Utf8Path> {
        let relative = if self.prefix == "." {
            path
        } else {
            path.strip_prefix(&self.prefix).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "glob match {path} does not start with capability prefix {}",
                        self.prefix
                    ),
                )
            })?
        };
        Ok(if relative.as_str().is_empty() {
            Utf8Path::new(".")
        } else {
            relative
        })
    }
}

/// Report whether `err` is how a link the capability cannot follow surfaces.
///
/// `cap_std` raises [`io::ErrorKind::PermissionDenied`] for a resolution that
/// leaves the capability's tree, and the platform raises
/// [`io::ErrorKind::NotFound`] for a link with no target. Nothing else counts:
/// a loop, an I/O failure, or an invalid argument all describe a tree that is
/// broken rather than a match that is simply not there.
fn is_unresolvable_link(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::PermissionDenied | io::ErrorKind::NotFound
    )
}

/// Characters [`super::normalize::force_literal_escapes`] wraps in a bracket
/// class to force a literal match.
const LITERAL_ESCAPES: [char; 6] = ['[', ']', '*', '?', '{', '}'];

/// The character a bracketed literal escape at the start of `rest` stands for,
/// paired with the escape's byte length.
///
/// Recognises exactly the six forms `force_literal_escapes` emits — `[*]`,
/// `[?]`, `[[]`, `[]]`, `[{]`, `[}]` — and nothing else. A genuine character
/// class such as `[ab]` is a wildcard and yields `None`, as does a
/// single-character class like `[a]`, which is treated conservatively even
/// though it matches only one character.
fn literal_escape(rest: &str) -> Option<(char, usize)> {
    let mut chars = rest.chars();
    if chars.next()? != '[' {
        return None;
    }
    let escaped = chars.next()?;
    if chars.next()? != ']' || !LITERAL_ESCAPES.contains(&escaped) {
        return None;
    }
    Some((
        escaped,
        '['.len_utf8() + escaped.len_utf8() + ']'.len_utf8(),
    ))
}

/// Byte offset of the first character that makes `normalized` a wildcard.
///
/// A bracketed literal escape is a literal character rather than a
/// metacharacter, so the scan steps over it instead of stopping there.
fn first_metacharacter(normalized: &str) -> usize {
    let mut idx = 0;
    while let Some(rest) = normalized.get(idx..) {
        let Some(next) = rest.chars().next() else {
            break;
        };
        if next == '[' {
            match literal_escape(rest) {
                Some((_, len)) => idx += len,
                None => return idx,
            }
        } else if matches!(next, '*' | '?' | '{') {
            return idx;
        } else {
            idx += next.len_utf8();
        }
    }
    normalized.len()
}

/// Longest literal directory prefix of a normalised pattern.
///
/// Scans up to the first glob metacharacter (`*`, `?`, `[`, `{`) and trims
/// back to the last path separator, yielding the deepest directory that the
/// pattern names literally. Bracketed literal escapes are stepped over, so
/// `src/[*]x/generated/*.c` reaches `src/[*]x/generated/` rather than stopping
/// at `src/`. Returns `.` when the pattern has no literal directory component.
///
/// The result is still pattern text: [`unescape_literal_escapes`] turns it
/// into the filesystem path it names.
pub(super) fn literal_dir_prefix(normalized: &str) -> &str {
    let meta_idx = first_metacharacter(normalized);
    let literal = normalized.get(..meta_idx).unwrap_or_default();
    // Keep the trailing separator so absolute roots stay absolute ("/").
    literal
        .rfind(std::path::MAIN_SEPARATOR)
        .and_then(|idx| literal.get(..=idx))
        .unwrap_or(".")
}

/// Resolve bracketed literal escapes to the characters they stand for.
///
/// The prefix names a directory to open, so `src/[*]x/` has to become the
/// path `src/*x/` before it reaches the filesystem — and before a match is
/// stripped of it, since the walker yields real paths.
pub(super) fn unescape_literal_escapes(prefix: &str) -> String {
    let mut out = String::with_capacity(prefix.len());
    let mut idx = 0;
    while let Some(rest) = prefix.get(idx..) {
        let Some(next) = rest.chars().next() else {
            break;
        };
        if let Some((escaped, len)) = literal_escape(rest) {
            out.push(escaped);
            idx += len;
        } else {
            out.push(next);
            idx += next.len_utf8();
        }
    }
    out
}

/// Open the directory used as the capability root for the glob.
///
/// Returns `Ok(None)` when the literal prefix does not exist (or is not a
/// directory); the pattern can match nothing in that case, mirroring the
/// empty result the matcher would produce.
pub(super) fn open_root_dir(pattern: &GlobPattern) -> io::Result<Option<GlobRoot>> {
    let prefix = unescape_literal_escapes(literal_dir_prefix(pattern.normalized()));
    match Dir::open_ambient_dir(&prefix, ambient_authority()) {
        Ok(dir) => Ok(Some(GlobRoot {
            dir,
            prefix: Utf8PathBuf::from(prefix),
        })),
        Err(err) if prefix_is_unopenable(&err) => {
            diagnostics::record_unopenable_prefix(pattern, &prefix);
            Ok(None)
        }
        Err(err) => Err(err),
    }
}

/// Report whether `err` means the literal prefix names no usable directory.
///
/// A missing path and a non-directory path both mean the pattern can match
/// nothing. On Windows `cap_primitives` signals the latter by constructing a
/// raw `ERROR_DIRECTORY`, so the raw code is matched alongside the portable
/// [`io::ErrorKind`]s rather than relying on the standard library's mapping of
/// that code. Every other failure — a genuine permission error, say — is left
/// to propagate.
fn prefix_is_unopenable(err: &io::Error) -> bool {
    if matches!(
        err.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
    ) {
        return true;
    }
    #[cfg(windows)]
    {
        /// `ERROR_DIRECTORY`: the path is not a directory.
        const ERROR_DIRECTORY: i32 = 267;
        return err.raw_os_error() == Some(ERROR_DIRECTORY);
    }
    #[cfg(not(windows))]
    false
}

fn create_io_error(pattern: &GlobPattern, position: usize, detail: String) -> Error {
    create_glob_error(
        &GlobErrorContext {
            pattern: pattern.raw().to_owned(),
            error_char: '\0',
            position,
            error_type: GlobErrorType::IoError,
        },
        Some(detail),
    )
}

/// Process a single glob entry, normalising UTF-8 paths and filtering
/// directories.
pub(super) fn process_glob_entry(
    entry: GlobEntryResult,
    pattern: &GlobPattern,
    root: &GlobRoot,
) -> std::result::Result<Option<String>, Error> {
    let path = entry.map_err(|e| create_io_error(pattern, 0, e.to_string()))?;
    let utf_path = Utf8PathBuf::try_from(path).map_err(|_| {
        create_io_error(
            pattern,
            pattern.raw().len(),
            "glob matched a non-UTF-8 path".to_owned(),
        )
    })?;
    let keep = names_a_file(root, &utf_path)
        .map_err(|err| create_io_error(pattern, pattern.raw().len(), err.to_string()))?;
    if !keep {
        return Ok(None);
    }
    Ok(Some(utf_path.as_str().replace('\\', "/")))
}

/// Report whether a match names a regular file reachable through the
/// capability, recording why it was dropped when it does not.
fn names_a_file(root: &GlobRoot, path: &Utf8Path) -> io::Result<bool> {
    // An unreachable match has already been recorded as skipped.
    let Some(metadata) = root.metadata(path)? else {
        return Ok(false);
    };
    if metadata.is_file() {
        return Ok(true);
    }
    diagnostics::record_not_a_file();
    Ok(false)
}
