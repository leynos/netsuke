//! Host path-spelling rules for a configured shell executable.
//!
//! A definition names the program a future adapter runs, and these predicates
//! decide only whether that spelling is a plausible single program reference: a
//! bare executable name or an absolute path on the loading host. Nothing here
//! probes the filesystem, so a definition naming a program that does not exist
//! is accepted at configuration time and fails only if something tries to run
//! it.
//!
//! The rules live beside [`ShellExecutable`] rather than inside it because they
//! are pure text inspection: they share no state with the type that retains an
//! accepted value, and both host families' rules are exercised on either build
//! host.
//!
//! [`ShellExecutable`]: super::ShellExecutable

use super::executable::ExecutableError;
use crate::shell_selection::HostFamily;

/// Check the rules shared by both host families, then the host's path rules.
pub(super) fn check_spelling(text: &str, host: HostFamily) -> Result<(), ExecutableError> {
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

/// Report whether `text` names a complete UNC share.
///
/// Two leading separators alone are not a share: `\\`, `//`, `\\server`, and
/// `\\server\` each name something unresolved, so they are reported as rooted
/// without a drive rather than accepted as absolute. A share needs a server and
/// a share name, which is what the two components after the separators supply.
fn is_unc_share(text: &str) -> bool {
    text.strip_prefix(['/', '\\'])
        .and_then(|rest| rest.strip_prefix(['/', '\\']))
        .is_some_and(names_server_and_share)
}

/// Report whether `remainder` starts with two non-empty path components.
fn names_server_and_share(remainder: &str) -> bool {
    let mut components = remainder.split(['/', '\\']);
    components.next().is_some_and(|server| !server.is_empty())
        && components.next().is_some_and(|share| !share.is_empty())
}

/// Report whether `character` separates Windows path components.
const fn is_windows_separator(character: char) -> bool {
    matches!(character, '/' | '\\')
}
