//! Pipeline status reporting for accessible and standard output modes.

use crate::output_prefs::OutputPrefs;
use std::io::{self, Write};
use std::sync::Mutex;

/// Thin wrapper for a 1-based stage index (no validation needed).
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StageNumber(u32);

impl StageNumber {
    /// Build a stage number without runtime validation.
    #[must_use]
    pub const fn new_unchecked(value: u32) -> Self {
        Self(value)
    }

    /// Return the raw numeric stage index.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Fluent localization key used for status output messages.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct LocalizationKey(&'static str);

impl LocalizationKey {
    /// Wrap a static Fluent key string.
    #[must_use]
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    /// Return the wrapped Fluent key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl From<&'static str> for LocalizationKey {
    fn from(s: &'static str) -> Self {
        Self::new(s)
    }
}

#[path = "status_pipeline.rs"]
mod pipeline;
#[path = "status_timing.rs"]
mod timing;
pub use pipeline::{PipelineStage, report_pipeline_stage};
pub use timing::VerboseTimingReporter;

#[path = "status_indicatif.rs"]
mod indicatif;
pub use self::indicatif::IndicatifReporter;
use self::indicatif::{format_completion_line, stage_label, task_progress_update};

/// Reports pipeline stage transitions and completion.
pub trait StatusReporter: Send + Sync {
    /// Emit a stage update.
    fn report_stage(&self, current: StageNumber, total: StageNumber, description: &str);
    /// Emit validated, monotonic task progress for Stage 6 execution.
    fn report_task_progress(&self, _current: u32, _total: u32, _description: &str) {}
    /// Emit a final completion message.
    fn report_complete(&self, tool_key: LocalizationKey);
}

/// Accessible reporter that emits prefixed, labelled lines to a writer.
///
/// The writer defaults to [`io::Stderr`]; tests can supply a `Vec<u8>`
/// via [`Self::with_writer`] for output capture.
pub struct AccessibleReporter<W: Write + Send = io::Stderr> {
    /// Output preferences controlling prefixes and indentation.
    prefs: OutputPrefs,
    /// The line sink, locked per write.
    writer: Mutex<W>,
}

impl AccessibleReporter {
    /// Create a reporter that writes to stderr.
    #[must_use]
    pub fn new(prefs: OutputPrefs) -> Self {
        Self {
            prefs,
            writer: Mutex::new(io::stderr()),
        }
    }
}

impl<W: Write + Send> AccessibleReporter<W> {
    /// Create a reporter that writes to the given sink.
    #[must_use]
    pub const fn with_writer(prefs: OutputPrefs, writer: W) -> Self {
        Self {
            prefs,
            writer: Mutex::new(writer),
        }
    }
}

impl<W: Write + Send> StatusReporter for AccessibleReporter<W> {
    fn report_stage(&self, current: StageNumber, total: StageNumber, description: &str) {
        let prefix = self.prefs.info_prefix();
        let message = stage_label(current, total, description);
        let mut w = self
            .writer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(writeln!(w, "{prefix} {message}"));
    }

    fn report_complete(&self, tool_key: LocalizationKey) {
        let line = format_completion_line(self.prefs, tool_key);
        let mut w = self
            .writer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(writeln!(w, "{line}"));
    }

    fn report_task_progress(&self, current: u32, total: u32, description: &str) {
        let message = task_progress_update(current, total, description);
        let mut w = self
            .writer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(writeln!(w, "{}{message}", self.prefs.task_indent()));
    }
}

/// Reporter that suppresses status output.
pub struct SilentReporter;

impl StatusReporter for SilentReporter {
    fn report_stage(&self, _current: StageNumber, _total: StageNumber, _description: &str) {}
    fn report_complete(&self, _tool_key: LocalizationKey) {}
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
// The test module reaches these through `use super::*`; expose them only in
// test builds so the production build neither warns nor carries their weight.
#[cfg(test)]
use self::indicatif::STAGE6_INDEX;
#[cfg(test)]
use self::pipeline::PIPELINE_STAGE_TOTAL;
#[cfg(test)]
use crate::localization::keys;
