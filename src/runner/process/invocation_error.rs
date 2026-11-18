//! Typed Ninja invocation failures and the compatibility bridge to `io::Error`.
//!
//! The public process helpers keep their `io::Result` contracts. Internally the
//! process core classifies a failure at the operation that produced it, because
//! an `io::ErrorKind` cannot distinguish a failed launch from another
//! `NotFound` source. The runner uses that classification to choose a
//! diagnostic instead of describing the temporary build file.

use camino::{Utf8Path, Utf8PathBuf};
use std::{io, process::ExitStatus};
use thiserror::Error;

/// Why a Ninja invocation failed.
#[derive(Debug, Error)]
pub(in crate::runner) enum NinjaInvocationError {
    /// The selected Ninja program could not be started.
    #[error("{source}")]
    Spawn {
        /// Program Netsuke asked the operating system to run.
        program: Utf8PathBuf,
        /// Original spawn error, preserved so its kind and OS code survive.
        source: io::Error,
    },
    /// Ninja started and reported an unsuccessful exit status.
    #[error("{0}")]
    Exit(#[source] NinjaExitError),
    /// Any other process input/output failure.
    #[error("{0}")]
    Io(#[from] io::Error),
}

impl NinjaInvocationError {
    /// Classify a failed launch of `program`.
    pub(in crate::runner) fn spawn(program: &Utf8Path, source: io::Error) -> Self {
        Self::Spawn {
            program: program.to_owned(),
            source,
        }
    }

    /// Classify an unsuccessful Ninja exit.
    pub(in crate::runner) const fn exit(error: NinjaExitError) -> Self {
        Self::Exit(error)
    }

    /// Classify a process failure that is neither a launch failure nor an exit.
    pub(in crate::runner) const fn io(source: io::Error) -> Self {
        Self::Io(source)
    }

    /// Convert the typed failure back into the public `io::Result` error shape.
    ///
    /// A spawn failure returns its original error, so `kind()` and
    /// `raw_os_error()` are unchanged. An unsuccessful exit keeps the legacy
    /// `ninja exited with …` text through [`NinjaExitError`].
    pub(in crate::runner) fn into_io_error(self) -> io::Error {
        match self {
            Self::Spawn { source, .. } | Self::Io(source) => source,
            Self::Exit(exit) => io::Error::other(exit),
        }
    }
}

/// An unsuccessful Ninja exit, retaining the legacy cause text.
///
/// This is the source of `RunnerError::NinjaFailed`, so a consumer can read the
/// exit status and operation without parsing the rendered cause.
#[derive(Debug, Error)]
#[error("ninja exited with {status}{attribution}")]
pub struct NinjaExitError {
    /// Full exit status, including a Unix signal when one terminated Ninja.
    status: ExitStatus,
    /// Invocation label: `build` or the Ninja tool name.
    operation: String,
    /// Pre-rendered command-list attribution suffix, empty when absent.
    ///
    /// The suffix form keeps the legacy cause text exactly one `Display`
    /// implementation, which the derive generates from the message above.
    attribution: String,
}

impl NinjaExitError {
    /// Construct an unsuccessful-exit failure.
    ///
    /// This is the supported way to build the source of
    /// [`RunnerError::NinjaFailed`](crate::runner::RunnerError::NinjaFailed)
    /// without running a process, which is what a caller reporting a recorded
    /// failure or a test fixture needs.
    #[must_use]
    pub fn new(status: ExitStatus, operation: &str, attribution: Option<String>) -> Self {
        Self {
            status,
            operation: operation.to_owned(),
            attribution: render_attribution(attribution),
        }
    }

    /// Return the exit status Ninja reported.
    #[must_use]
    pub const fn status(&self) -> ExitStatus {
        self.status
    }

    /// Return the operation label: `build` or the Ninja tool name.
    #[must_use]
    pub fn operation(&self) -> &str {
        &self.operation
    }
}

/// Render an optional attribution as the suffix the legacy cause text appends.
///
/// `map_or_else` rather than `Option::map_or_default`, because the Whitaker
/// gate compiles this crate with an older nightly that still gates that method.
fn render_attribution(attribution: Option<String>) -> String {
    attribution.map_or_else(String::new, |text| format!(": {text}"))
}

#[cfg(test)]
mod tests {
    //! Characterize the legacy cause text preserved by the bridge.

    use super::*;

    /// Build one unsuccessful exit with the supplied attribution.
    fn exit_error(attribution: Option<&str>) -> NinjaExitError {
        NinjaExitError::new(exit_status(7), "build", attribution.map(str::to_owned))
    }

    #[cfg(unix)]
    fn exit_status(code: i32) -> ExitStatus {
        use std::os::unix::process::ExitStatusExt;

        ExitStatus::from_raw(code << 8)
    }

    #[cfg(windows)]
    fn exit_status(code: u32) -> ExitStatus {
        use std::os::windows::process::ExitStatusExt;

        ExitStatus::from_raw(code)
    }

    #[test]
    fn exit_display_keeps_the_legacy_text_without_attribution() {
        let rendered = exit_error(None).to_string();

        assert_eq!(
            rendered,
            format!("ninja exited with {}", exit_status(7)),
            "an exit without attribution must keep the legacy cause text"
        );
    }

    #[test]
    fn exit_display_appends_the_attribution() {
        let rendered = exit_error(Some("action abc, entry 2")).to_string();

        assert_eq!(
            rendered,
            format!("ninja exited with {}: action abc, entry 2", exit_status(7)),
            "an exit with attribution must append it to the legacy cause text"
        );
    }

    #[test]
    fn spawn_failure_preserves_the_original_io_error() {
        let failure = NinjaInvocationError::spawn(
            Utf8Path::new("ninja"),
            io::Error::new(io::ErrorKind::NotFound, "missing"),
        );

        let converted = failure.into_io_error();

        assert_eq!(
            converted.kind(),
            io::ErrorKind::NotFound,
            "conversion must preserve the spawn error kind"
        );
        assert_eq!(
            converted.to_string(),
            "missing",
            "conversion must return the original error text"
        );
    }
}
