//! Detail types and message-append helpers for command failure rendering.
//!
//! Kept separate from [`super`]'s error module so `error.rs` stays within the
//! repository's 400-line cap. These types describe what the failing process
//! reported and how its exit status and stderr suffix the localized message.

use super::super::config::{OutputMode, OutputStream};
use crate::localization::{self, keys};

/// The exit status and captured stderr a failing process reported.
#[derive(Clone, Copy)]
pub(super) struct ExitDetails<'a> {
    /// Exit status (`None` when terminated by a signal).
    pub(super) status: Option<i32>,
    /// Captured stderr bytes.
    pub(super) stderr: &'a [u8],
}

impl<'a> ExitDetails<'a> {
    /// Wrap an optional exit status and captured stderr.
    pub(super) const fn new(status: Option<i32>, stderr: &'a [u8]) -> Self {
        Self { status, stderr }
    }
}

/// The output-stream constraint that was exceeded.
#[derive(Clone, Copy)]
pub(super) struct LimitExceeded {
    /// Which pipe exceeded the budget.
    pub(super) stream: OutputStream,
    /// Whether capture or streaming mode was active.
    pub(super) mode: OutputMode,
    /// The configured byte ceiling that was exceeded.
    pub(super) limit: u64,
}

impl LimitExceeded {
    /// Record the stream, mode, and ceiling of an exceeded output budget.
    pub(super) const fn new(stream: OutputStream, mode: OutputMode, limit: u64) -> Self {
        Self {
            stream,
            mode,
            limit,
        }
    }
}

/// Append an exit-status (or signal) suffix to a rendered message.
pub(super) fn append_exit_status(message: &mut String, status: Option<i32>) {
    if let Some(code) = status {
        let suffix = localization::message(keys::COMMAND_EXIT_STATUS_SUFFIX)
            .with_arg("status", code)
            .to_string();
        message.push(' ');
        message.push_str(&suffix);
    } else {
        message.push(' ');
        message.push_str(&localization::message(keys::COMMAND_SIGNAL_SUFFIX).to_string());
    }
}

/// Append captured stderr to a rendered message when it carries text.
pub(super) fn append_stderr(message: &mut String, stderr: &[u8]) {
    let stderr_text = String::from_utf8_lossy(stderr);
    let trimmed = stderr_text.trim();
    if !trimmed.is_empty() {
        message.push_str(": ");
        message.push_str(trimmed);
    }
}
