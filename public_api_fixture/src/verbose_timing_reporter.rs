//! Type-check verbose timing reporter construction.
//!
//! Both the default standard-error constructor and the public generic writer
//! constructor must be available to an external embedder.

use netsuke::output_prefs::resolve_with;
use netsuke::status::{SilentReporter, VerboseTimingReporter};

/// Construct the reporter with its default standard-error writer.
#[must_use]
pub fn stderr_reporter() -> VerboseTimingReporter {
    VerboseTimingReporter::new(Box::new(SilentReporter), resolve_with(None, |_| None))
}

/// Construct the reporter around a caller-supplied writer.
#[must_use]
pub fn buffered_reporter() -> VerboseTimingReporter<Vec<u8>> {
    VerboseTimingReporter::with_writer(
        Box::new(SilentReporter),
        resolve_with(None, |_| None),
        Vec::new(),
    )
}
