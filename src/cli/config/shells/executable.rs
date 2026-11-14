//! Retain the validated `executable` field of a trusted shell definition.
//!
//! The spelling rules themselves live in [`super::spelling`]; this module owns
//! the type that holds an accepted value, its serde surface, and the error
//! vocabulary those rules report. Host semantics are an explicit parameter
//! rather than `cfg!(windows)`, so both families' rules are exercised on either
//! build host.

use super::preview::preview;
use super::spelling::check_spelling;
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
    #[error("a Windows shell executable must not name a drive without a root, as in `C:tool`")]
    DriveRelative,
    /// Reject a Windows path rooted without a drive letter or share.
    #[error("a Windows shell executable must name a drive or a UNC share")]
    RootedWithoutDrive,
}
