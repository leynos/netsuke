//! Shared capture and assertion helpers for the configuration discovery tests.
//!
//! [`capture_events`] runs a closure under a capturing subscriber and
//! [`find_event`] locates one emitted event, so the tracing and layer test
//! modules share a single implementation of both.
//!
//! [`EventAssertion`] pairs a captured tracing event with the path under
//! assertion so the discovery tests can check bounded field rendering, privacy,
//! and snapshot normalization without threading repeated `&str`/`&Path`
//! arguments through every helper.

use crate::test_tracing_capture::with_test_subscriber;
use anyhow::{Context, Result, ensure};
use std::path::Path;
use tracing_subscriber::filter::LevelFilter;

use super::diagnostics::path_hash;

/// Run `test` under a TRACE-level capturing subscriber.
///
/// Returns the closure's value alongside every event it emitted, so a test can
/// assert on both the outcome and its instrumentation.
pub(super) fn capture_events<T, E>(
    test: impl FnOnce() -> std::result::Result<T, E>,
) -> std::result::Result<(T, Vec<String>), E> {
    with_test_subscriber(LevelFilter::TRACE, |captured| {
        let value = test()?;
        Ok((value, captured.snapshot()))
    })
}

/// Return the first captured event containing `message`.
///
/// Fails with the full event list when no event matches, so assertion failures
/// show what was actually emitted.
pub(super) fn find_event<'a>(events: &'a [String], message: &str) -> Result<&'a String> {
    events
        .iter()
        .find(|event| event.contains(message))
        .with_context(|| format!("expected event containing {message:?} in {events:?}"))
}

/// Bundles a captured tracing event with the path under assertion so the
/// discovery-test helpers share that context instead of threading repeated
/// `&str`/`&Path` arguments through every call.
pub(super) struct EventAssertion<'a> {
    event: &'a str,
    path: &'a Path,
}

impl<'a> EventAssertion<'a> {
    /// Bind `event` and `path` together for the assertion helpers below.
    pub(super) fn new(event: &'a str, path: &'a Path) -> Self {
        Self { event, path }
    }

    /// Assert the event carries the bounded `path_hash` and `path_file_name`
    /// fields for the path in any accepted rendering form.
    pub(super) fn ensure_bounded_path_fields(&self) -> Result<()> {
        let hash = path_hash(self.path);
        let file_name = self
            .path
            .file_name()
            .with_context(|| format!("expected file name for {}", self.path.display()))?;
        ensure!(
            self.event.contains(&format!("path_hash=\"{hash}\""))
                || self.event.contains(&format!("path_hash=Some(\"{hash}\")"))
                || self.event.contains(&format!("path_hash={hash}")),
            "event should include path hash for {}: {}",
            self.path.display(),
            self.event
        );
        ensure!(
            self.event
                .contains(&format!("path_file_name=Some({file_name:?})")),
            "event should include path file name for {}: {}",
            self.path.display(),
            self.event
        );
        Ok(())
    }

    /// Assert the event never leaks the raw path string.
    pub(super) fn ensure_raw_path_absent(&self) -> Result<()> {
        ensure!(
            !self.event.contains(self.path.to_string_lossy().as_ref()),
            "event should not include raw path {}: {}",
            self.path.display(),
            self.event
        );
        Ok(())
    }

    /// Assert the event leaks neither the raw path nor the `formatted_error`.
    pub(super) fn ensure_private_event_fields(&self, formatted_error: &str) -> Result<()> {
        self.ensure_raw_path_absent()?;
        ensure!(
            !self.event.contains(formatted_error),
            "event should not include formatted error text: {}",
            self.event
        );
        Ok(())
    }

    /// Return the event with the runtime path hash replaced by a stable
    /// placeholder, after asserting the bounded fields are present.
    ///
    /// The caller performs the `insta` assertion so snapshot names stay bound to
    /// the test module rather than this helper module.
    pub(super) fn normalize_path_hash(&self) -> Result<String> {
        self.ensure_bounded_path_fields()?;
        Ok(self.event.replace(&path_hash(self.path), "[path_hash]"))
    }
}
