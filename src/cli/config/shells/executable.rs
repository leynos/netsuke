//! Validate the `executable` field of a trusted shell definition.
//!
//! A definition names the program a future adapter runs, and this module decides
//! only whether that spelling is a plausible single program reference: a bare
//! executable name or an absolute path on the loading host. Nothing here probes
//! the filesystem, so a definition naming a program that does not exist is
//! accepted at configuration time and fails only if something tries to run it.
//!
//! Host semantics are an explicit parameter rather than `cfg!(windows)`, so both
//! families' rules are exercised on either build host.

use super::preview::preview;
use crate::shell_selection::HostFamily;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use thiserror::Error;

/// Retain an accepted executable spelling exactly as it was configured.
///
/// No expansion, normalisation, trimming, or resolution happens, so the value a
/// definition declares is the value a future adapter receives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellExecutable(String);

impl ShellExecutable {
    /// Validate `text` as a bare program name or an absolute path on `host`.
    ///
    /// # Errors
    ///
    /// Returns the first violated rule: empty input, an embedded NUL, the `.` or
    /// `..` directory references, a relative path, a drive-relative Windows
    /// path, or a Windows path rooted without a drive.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::cli::config::shells::ShellExecutable;
    /// use netsuke::shell_selection::HostFamily;
    /// assert!(ShellExecutable::parse("dash", HostFamily::UnixLike).is_ok());
    /// assert!(ShellExecutable::parse("/bin/dash", HostFamily::UnixLike).is_ok());
    /// assert!(ShellExecutable::parse("bin/dash", HostFamily::UnixLike).is_err());
    /// ```
    pub fn parse(text: &str, host: HostFamily) -> Result<Self, ExecutableError> {
        check_spelling(text, host)?;
        Ok(Self(text.to_owned()))
    }

    /// Borrow the accepted spelling exactly as configured.
    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for ShellExecutable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for ShellExecutable {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ShellExecutable {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text, HostFamily::current()).map_err(|error| {
            serde::de::Error::custom(format!("{error}; value is {}", preview(&text)))
        })
    }
}

/// Check the rules shared by both host families, then the host's path rules.
fn check_spelling(text: &str, host: HostFamily) -> Result<(), ExecutableError> {
    if text.is_empty() {
        return Err(ExecutableError::Empty);
    }
    if text.contains('\0') {
        return Err(ExecutableError::Nul);
    }
    if matches!(text, "." | "..") {
        return Err(ExecutableError::DirectoryReference);
    }
    match host {
        HostFamily::UnixLike => check_unix_spelling(text),
        HostFamily::Windows => check_windows_spelling(text),
    }
}

/// Accept a rooted path or a bare name; reject any other spelling with a slash.
fn check_unix_spelling(text: &str) -> Result<(), ExecutableError> {
    if text.starts_with('/') || !text.contains(['/', '\\']) {
        Ok(())
    } else {
        Err(ExecutableError::RelativePath)
    }
}

/// Apply Windows path semantics, where both separators are significant.
///
/// The tests run in this order so each rejected spelling reports its most
/// specific reason: a drive with a root is absolute, a drive without one is
/// drive-relative, a rooted path with no drive is unresolved, and anything else
/// carrying a separator is relative.
fn check_windows_spelling(text: &str) -> Result<(), ExecutableError> {
    if is_windows_absolute(text) {
        Ok(())
    } else if has_drive_prefix(text) {
        Err(ExecutableError::DriveRelative)
    } else if text.starts_with(['/', '\\']) {
        Err(ExecutableError::RootedWithoutDrive)
    } else if text.contains(['/', '\\']) {
        Err(ExecutableError::RelativePath)
    } else {
        Ok(())
    }
}

/// Report whether `text` is an absolute Windows path: a drive or a UNC share.
fn is_windows_absolute(text: &str) -> bool {
    has_drive_absolute(text) || is_unc_share(text)
}

/// Report whether `text` begins with an ASCII drive letter and its root.
fn has_drive_absolute(text: &str) -> bool {
    let mut characters = text.chars();
    matches!(
        (characters.next(), characters.next(), characters.next()),
        (Some(drive), Some(':'), Some(separator))
            if drive.is_ascii_alphabetic() && is_windows_separator(separator)
    )
}

/// Report whether `text` begins with an ASCII drive letter and a colon only.
fn has_drive_prefix(text: &str) -> bool {
    let mut characters = text.chars();
    matches!(
        (characters.next(), characters.next()),
        (Some(drive), Some(':')) if drive.is_ascii_alphabetic()
    )
}

/// Report whether `text` begins with the two separators of a UNC share.
fn is_unc_share(text: &str) -> bool {
    let mut characters = text.chars();
    matches!(
        (characters.next(), characters.next()),
        (Some(first), Some(second))
            if is_windows_separator(first) && is_windows_separator(second)
    )
}

/// Report whether `character` separates Windows path components.
const fn is_windows_separator(character: char) -> bool {
    matches!(character, '/' | '\\')
}

/// Classify the executable-spelling rules a definition may violate.
///
/// Each variant states the rule, not the offending value: the caller adds a
/// bounded preview so the raw spelling is escaped exactly once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum ExecutableError {
    /// Reject a missing value.
    #[error("a shell executable must not be empty")]
    Empty,
    /// Reject embedded NUL, which no process argument can carry.
    #[error("a shell executable must not contain a NUL character")]
    Nul,
    /// Reject the two directory references, which name no program.
    #[error("a shell executable must be a program name, not `.` or `..`")]
    DirectoryReference,
    /// Reject a path with a separator that is not rooted.
    #[error("a shell executable must be a bare name or an absolute path, not a relative path")]
    RelativePath,
    /// Reject a Windows path that names a drive without its root.
    #[error(
        "a Windows shell executable must not name a drive without a root, as in `C:\\tools\\shell.exe`"
    )]
    DriveRelative,
    /// Reject a Windows path rooted without a drive letter or share.
    #[error("a Windows shell executable must name a drive or a UNC share")]
    RootedWithoutDrive,
}
