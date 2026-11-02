//! Locate Windows PowerShell without consulting executable search order.
//!
//! Windows resolves an unqualified `powershell.exe` through the application,
//! current, and system directories before `PATH`, so a workspace file can win
//! that race and inherit the build process's authority (CWE-427). This module
//! therefore asks the Known Folder API for the system directory, appends the
//! fixed Windows PowerShell engine path, and accepts only an absolute UTF-8
//! path to a regular file that is not a reparse point.
//!
//! The lookup deliberately never reads `PATH`, the current directory,
//! `SystemRoot`, `WINDIR`, or `PSHOME`, never tries `pwsh.exe`, and never
//! launches a candidate to identify it. Binding authenticates the location at
//! generation time; it cannot prevent the file being replaced before Ninja
//! launches it.

use std::io;
use std::path::PathBuf;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::{Dir, Metadata};
use thiserror::Error;

use super::{InterpreterExecutable, InterpreterExecutableError};

/// Directory of the Windows PowerShell engine beneath the system directory.
const WINDOWS_POWER_SHELL_DIRECTORY: &str = "WindowsPowerShell/v1.0";

/// Executable name inside the Windows PowerShell engine directory.
const WINDOWS_POWER_SHELL_EXECUTABLE: &str = "powershell.exe";

/// Supply the Windows system directory for one PowerShell lookup.
///
/// The trait exists so tests can supply a fixture directory without changing
/// process environment variables or relying on a Windows host.
pub(crate) trait SystemDirectory {
    /// Return the trusted Windows system directory, when the host has one.
    fn system_directory(&self) -> Option<PathBuf>;
}

/// Production provider backed by the Windows Known Folder API.
///
/// Non-Windows hosts have no Windows system directory, so this provider
/// returns `None` there and the locator is never used in production.
pub(crate) struct KnownFolderSystemDirectory;

impl SystemDirectory for KnownFolderSystemDirectory {
    #[cfg(windows)]
    fn system_directory(&self) -> Option<PathBuf> {
        known_folders::get_known_folder_path(known_folders::KnownFolder::System)
    }

    #[cfg(not(windows))]
    fn system_directory(&self) -> Option<PathBuf> {
        None
    }
}

/// Failures while locating the trusted Windows PowerShell executable.
#[derive(Debug, Error)]
pub(crate) enum WindowsPowerShellLocationError {
    /// The host did not provide a Windows system directory.
    #[error("the Windows system directory is unavailable")]
    SystemDirectoryUnavailable,
    /// The supplied system directory is not valid UTF-8.
    #[error("the Windows system directory is not valid UTF-8: {path}")]
    NonUtf8SystemDirectory {
        /// Directory path that could not be represented as UTF-8.
        path: PathBuf,
    },
    /// The system directory could not anchor an absolute executable path.
    #[error("the Windows system directory is not absolute: {path}")]
    RelativeSystemDirectory {
        /// Directory supplied by the provider.
        path: Utf8PathBuf,
    },
    /// Windows PowerShell is absent at the trusted path.
    #[error("Windows PowerShell is missing at {expected}")]
    Missing {
        /// Trusted path the locator expected to find.
        expected: Utf8PathBuf,
        /// Underlying inspection failure.
        #[source]
        source: io::Error,
    },
    /// The trusted path exists but does not name a regular file.
    #[error("Windows PowerShell at {expected} is not a regular file")]
    NotRegularFile {
        /// Trusted path the locator inspected.
        expected: Utf8PathBuf,
    },
    /// The trusted path could not be inspected.
    #[error("could not inspect Windows PowerShell at {expected}")]
    Inspect {
        /// Trusted path the locator inspected.
        expected: Utf8PathBuf,
        /// Underlying inspection failure.
        #[source]
        source: io::Error,
    },
    /// The trusted path cannot serve as an executable binding.
    #[error("Windows PowerShell path {path} is not a valid executable binding")]
    InvalidExecutablePath {
        /// Trusted path the locator constructed.
        path: Utf8PathBuf,
        /// Constructor rejection.
        #[source]
        source: InterpreterExecutableError,
    },
}

impl WindowsPowerShellLocationError {
    /// Borrow the expected executable path when the failure names one.
    ///
    /// Providers that cannot supply a directory have no full path to name, so
    /// callers fall back to describing the fixed relative path instead.
    pub(crate) fn expected_path(&self) -> Option<&Utf8Path> {
        match self {
            Self::Missing { expected, .. }
            | Self::NotRegularFile { expected }
            | Self::Inspect { expected, .. } => Some(expected),
            Self::InvalidExecutablePath { path, .. } => Some(path),
            Self::SystemDirectoryUnavailable
            | Self::NonUtf8SystemDirectory { .. }
            | Self::RelativeSystemDirectory { .. } => None,
        }
    }
}

/// Locate the trusted Windows PowerShell executable for generated recipes.
///
/// # Errors
///
/// Returns a typed failure when the provider has no usable system directory,
/// the expected file is missing or is not a regular non-reparse file, or the
/// constructed path cannot be bound as an [`InterpreterExecutable`].
pub(crate) fn locate_windows_power_shell(
    provider: &impl SystemDirectory,
) -> Result<InterpreterExecutable, WindowsPowerShellLocationError> {
    let raw_system_directory = provider
        .system_directory()
        .ok_or(WindowsPowerShellLocationError::SystemDirectoryUnavailable)?;
    let system_directory = Utf8PathBuf::from_path_buf(raw_system_directory)
        .map_err(|path| WindowsPowerShellLocationError::NonUtf8SystemDirectory { path })?;
    if !system_directory.is_absolute() {
        return Err(WindowsPowerShellLocationError::RelativeSystemDirectory {
            path: system_directory,
        });
    }

    let engine_directory = system_directory.join(WINDOWS_POWER_SHELL_DIRECTORY);
    let expected = engine_directory.join(WINDOWS_POWER_SHELL_EXECUTABLE);
    let handle = Dir::open_ambient_dir(&engine_directory, ambient_authority())
        .map_err(|source| inspection_error(&expected, source))?;
    // `symlink_metadata` does not follow the final component, so a symlink is
    // refused as a reparse point instead of being inspected through its target.
    let metadata = handle
        .symlink_metadata(WINDOWS_POWER_SHELL_EXECUTABLE)
        .map_err(|source| inspection_error(&expected, source))?;
    if !metadata.is_file() || has_reparse_attribute(&metadata) {
        return Err(WindowsPowerShellLocationError::NotRegularFile { expected });
    }

    InterpreterExecutable::from_absolute_path(expected.clone()).map_err(|source| {
        WindowsPowerShellLocationError::InvalidExecutablePath {
            path: expected,
            source,
        }
    })
}

/// Classify a failed directory or file inspection against one expected path.
fn inspection_error(expected: &Utf8Path, source: io::Error) -> WindowsPowerShellLocationError {
    match source.kind() {
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory => {
            WindowsPowerShellLocationError::Missing {
                expected: expected.to_owned(),
                source,
            }
        }
        _ => WindowsPowerShellLocationError::Inspect {
            expected: expected.to_owned(),
            source,
        },
    }
}

/// Return whether the metadata describes a reparse point.
///
/// The check uses the same attribute policy as the manifest's path filters, so
/// a junction, symlink, or any other reparse tag is refused rather than
/// followed.
#[cfg(windows)]
fn has_reparse_attribute(metadata: &Metadata) -> bool {
    use cap_std::fs::MetadataExt as _;

    crate::stdlib::path::windows_reparse::is_prohibited_reparse_point(metadata.file_attributes())
}

/// Non-Windows hosts have no Windows reparse attributes to inspect.
#[cfg(not(windows))]
const fn has_reparse_attribute(_metadata: &Metadata) -> bool {
    false
}

#[cfg(test)]
#[path = "locator_tests.rs"]
mod tests;
