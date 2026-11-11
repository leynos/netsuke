//! Classify child-output forwarding failures and bound their telemetry.
//!
//! Every forwarding failure reaches a [`ForwardFailureKind`] before it is
//! counted or logged. The metric labels are bounded: [`ForwardStream::label`]
//! and [`ForwardFailureKind::label`] choose values from closed arrays, and the
//! application recorder admits exactly the same arrays, so a new label value
//! cannot reach the retained snapshot unless it is declared here first.
//!
//! The warning event is bounded differently. Its `stream` and `failure_kind`
//! fields carry the same closed values, but its `error` field carries the
//! underlying I/O error, which may quote an operating-system message such as a
//! destination path. That field is deliberately kept out of the metric labels:
//! it is what makes a lost stream diagnosable, and it is not a value the
//! recorder admits.

use metrics::{counter, describe_counter};
use std::{io, sync::Once};

/// Counter recording classified child-output forwarding failures.
pub const FORWARD_FAILURES_TOTAL: &str = "netsuke_ninja_forward_failures_total";

/// Bounded `stream` label values accepted for forwarding-failure telemetry.
pub const FORWARD_STREAM_VALUES: [&str; 2] = ["stdout", "stderr"];

/// Bounded `failure_kind` label values accepted for forwarding-failure telemetry.
pub const FORWARD_FAILURE_KIND_VALUES: [&str; 4] = ["read", "write", "flush", "drain"];

/// Complete labelled series admitted for the forwarding-failure counter.
///
/// The recorder admits this pairing directly, so a label name and the
/// vocabulary behind it cannot drift apart from the arrays declared above.
pub const FORWARD_FAILURE_LABELS: [(&str, &[&str]); 2] = [
    ("stream", &FORWARD_STREAM_VALUES),
    ("failure_kind", &FORWARD_FAILURE_KIND_VALUES),
];

/// One child stream whose output the process layer forwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::runner::process) enum ForwardStream {
    /// The child's standard output.
    Stdout,
    /// The child's standard error.
    Stderr,
}

impl ForwardStream {
    /// Return the bounded `stream` metric label for this stream.
    pub(in crate::runner::process) const fn label(self) -> &'static str {
        match self {
            Self::Stdout => FORWARD_STREAM_VALUES[0],
            Self::Stderr => FORWARD_STREAM_VALUES[1],
        }
    }
}

/// Bounded classification of a child-output forwarding failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::runner::process) enum ForwardFailureKind {
    /// The child stream could not be read.
    Read,
    /// The destination rejected a chunk.
    Write,
    /// The destination rejected the flush that publishes a chunk.
    Flush,
    /// Remaining child output could not be drained after a failure.
    Drain,
}

impl ForwardFailureKind {
    /// Return the bounded `failure_kind` metric label for this classification.
    pub(in crate::runner::process) const fn label(self) -> &'static str {
        match self {
            Self::Read => FORWARD_FAILURE_KIND_VALUES[0],
            Self::Write => FORWARD_FAILURE_KIND_VALUES[1],
            Self::Flush => FORWARD_FAILURE_KIND_VALUES[2],
            Self::Drain => FORWARD_FAILURE_KIND_VALUES[3],
        }
    }
}

/// A classified forwarding failure paired with the underlying I/O error.
///
/// The error is retained only for the warning event's `error` field; it never
/// reaches a metric label.
#[derive(Debug)]
pub(in crate::runner::process) struct ForwardFailure {
    /// Classification reported through telemetry.
    pub(in crate::runner::process) kind: ForwardFailureKind,
    /// Original error, reported in the warning event.
    pub(in crate::runner::process) source: io::Error,
}

/// Narrow sink receiving each classified forwarding failure.
///
/// The copy loop only classifies what went wrong; the process boundary decides
/// what a failure means. Injecting the reporter keeps metric emission and
/// warning policy out of the byte-moving code, and lets a test observe the
/// reporting contract through an explicit dependency instead of a global
/// recorder.
pub(in crate::runner::process) type FailureReporter<'a> =
    &'a mut dyn FnMut(ForwardStream, &ForwardFailure);

/// Pair one forwarded stream's identity with the reporter that hears about it.
///
/// The two always travel together: the stream name exists only to label a
/// failure, and the reporter is its sole consumer. Bundling them keeps the copy
/// helpers inside the argument budget, and makes it impossible to hand a
/// reporter the label of the other stream.
pub(in crate::runner::process) struct ForwardRoute<'a> {
    /// Child stream these bytes came from.
    stream: ForwardStream,
    /// Sink receiving each classified failure on that stream.
    report: FailureReporter<'a>,
}

impl<'a> ForwardRoute<'a> {
    /// Pair a stream identity with the reporter for that stream's failures.
    pub(in crate::runner::process) const fn new(
        stream: ForwardStream,
        report: FailureReporter<'a>,
    ) -> Self {
        Self { stream, report }
    }

    /// Hand one classified failure to the reporter, labelled with the stream.
    pub(super) fn report(&mut self, failure: &ForwardFailure) {
        (self.report)(self.stream, failure);
    }
}

impl ForwardFailure {
    /// Classify a failure to read the child stream.
    pub(super) const fn read(source: io::Error) -> Self {
        Self {
            kind: ForwardFailureKind::Read,
            source,
        }
    }

    /// Classify a failure to write a chunk to the destination.
    pub(super) const fn write(source: io::Error) -> Self {
        Self {
            kind: ForwardFailureKind::Write,
            source,
        }
    }

    /// Classify a failure to flush a written chunk to the destination.
    pub(super) const fn flush(source: io::Error) -> Self {
        Self {
            kind: ForwardFailureKind::Flush,
            source,
        }
    }

    /// Classify a failure to drain output after a forwarding failure.
    pub(super) const fn drain(source: io::Error) -> Self {
        Self {
            kind: ForwardFailureKind::Drain,
            source,
        }
    }
}

/// Describe the forwarding-failure counter once per process.
fn describe_metric() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            FORWARD_FAILURES_TOTAL,
            "Counts child-output forwarding failures by stream and failure kind."
        );
    });
}

/// Record one classified forwarding failure under bounded labels.
pub(in crate::runner::process) fn record_failure(stream: ForwardStream, kind: ForwardFailureKind) {
    describe_metric();
    counter!(
        FORWARD_FAILURES_TOTAL,
        "stream" => stream.label(),
        "failure_kind" => kind.label()
    )
    .increment(1);
}

#[cfg(test)]
mod tests {
    //! Verify the label vocabularies stay closed and distinct.

    use super::{
        FORWARD_FAILURE_KIND_VALUES, FORWARD_STREAM_VALUES, ForwardFailureKind, ForwardStream,
    };

    #[test]
    fn labels_are_drawn_from_the_declared_vocabularies() {
        assert_eq!(ForwardStream::Stdout.label(), FORWARD_STREAM_VALUES[0]);
        assert_eq!(ForwardStream::Stderr.label(), FORWARD_STREAM_VALUES[1]);
        for (kind, expected) in [
            (ForwardFailureKind::Read, FORWARD_FAILURE_KIND_VALUES[0]),
            (ForwardFailureKind::Write, FORWARD_FAILURE_KIND_VALUES[1]),
            (ForwardFailureKind::Flush, FORWARD_FAILURE_KIND_VALUES[2]),
            (ForwardFailureKind::Drain, FORWARD_FAILURE_KIND_VALUES[3]),
        ] {
            assert_eq!(kind.label(), expected);
        }
    }
}
