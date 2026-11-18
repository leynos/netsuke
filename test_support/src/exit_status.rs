//! Build a `std::process::ExitStatus` from an exit code without spawning.
//!
//! Unix stores the exit code in the high byte of a raw wait status while
//! Windows stores it directly. Taking a `u8` keeps the caller's value
//! identical on both platforms, so only this adapter differs between them.

use std::process::ExitStatus;

/// Report an exit status without spawning a process.
///
/// # Examples
///
/// ```
/// use test_support::exit_status;
///
/// let status = exit_status(7);
/// assert_eq!(status.code(), Some(7), "the adapter should keep the exit code");
/// ```
#[cfg(unix)]
#[must_use]
pub fn exit_status(code: u8) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;

    ExitStatus::from_raw(i32::from(code) << 8)
}

/// Report an exit status without spawning a process.
#[cfg(windows)]
#[must_use]
pub fn exit_status(code: u8) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;

    ExitStatus::from_raw(u32::from(code))
}
