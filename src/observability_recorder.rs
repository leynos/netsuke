//! Bounded process recorder for configuration observability.

use super::{
    CONFIG_LOAD_COUNTER, CONFIG_LOAD_DURATION, DIAG_MODE_PHASE, MERGE_PHASE,
    STARTUP_CONFIG_LOAD_COUNTER, STARTUP_CONFIG_LOAD_DURATION,
};
use metrics::{Counter, Gauge, Histogram, Key, KeyName, Metadata, SharedString, Unit};
use metrics_util::MetricKind;
use metrics_util::debugging::{DebuggingRecorder, Snapshotter};

/// Label key naming the configuration-load phase on every series.
const PHASE_LABEL: &str = "phase";
/// Label key naming the outcome on configuration-load counter series.
const OUTCOME_LABEL: &str = "outcome";
/// The bounded phase values accepted on every configuration-load series.
const PHASE_VALUES: [&str; 2] = [DIAG_MODE_PHASE, MERGE_PHASE];
/// The bounded outcome values accepted on configuration-load counter series.
const OUTCOME_VALUES: [&str; 2] = ["success", "failure"];

/// Application recorder that retains only bounded configuration-load metrics.
///
/// The process-wide debugging recorder is a shutdown-only diagnostic aid. It
/// must not retain workload-proportional observations from unrelated metrics.
#[derive(Debug)]
pub(super) struct ConfigMetricsRecorder {
    /// Inner recorder storing accepted observations for later snapshots.
    inner: DebuggingRecorder,
}

impl ConfigMetricsRecorder {
    /// Build a recorder over a fresh debugging recorder.
    pub(super) fn new() -> Self {
        Self {
            inner: DebuggingRecorder::new(),
        }
    }

    /// Return a snapshotter draining the recorder's observations.
    pub(super) fn snapshotter(&self) -> Snapshotter {
        self.inner.snapshotter()
    }

    /// Name filtering for describes, which carry no labels to validate.
    fn accepts_name(name: &str) -> bool {
        matches!(
            name,
            CONFIG_LOAD_COUNTER
                | CONFIG_LOAD_DURATION
                | STARTUP_CONFIG_LOAD_COUNTER
                | STARTUP_CONFIG_LOAD_DURATION
        )
    }

    /// Admit only the exact bounded series expected of `kind`.
    ///
    /// Rejects gauges (neither vocabulary name is a gauge) and same-name
    /// series whose label set is missing, extra, or unbounded.
    fn accepts_registration(key: &Key, kind: MetricKind) -> bool {
        match kind {
            MetricKind::Counter => {
                (key.name() == CONFIG_LOAD_COUNTER
                    && exact_labels(
                        key,
                        &[
                            (PHASE_LABEL, &PHASE_VALUES),
                            (OUTCOME_LABEL, &OUTCOME_VALUES),
                        ],
                    ))
                    || (key.name() == STARTUP_CONFIG_LOAD_COUNTER
                        && exact_labels(key, &[(OUTCOME_LABEL, &OUTCOME_VALUES)]))
            }
            MetricKind::Histogram => {
                (key.name() == CONFIG_LOAD_DURATION
                    && exact_labels(key, &[(PHASE_LABEL, &PHASE_VALUES)]))
                    || (key.name() == STARTUP_CONFIG_LOAD_DURATION && exact_labels(key, &[]))
            }
            MetricKind::Gauge => false,
        }
    }

    /// Route an accepted operation to the inner recorder and a rejected one
    /// to `reject`, so registration failures yield noop handles and describes
    /// of foreign names stay silent.
    fn forward<R>(
        &self,
        accepted: bool,
        reject: impl FnOnce() -> R,
        accept: impl FnOnce(&DebuggingRecorder) -> R,
    ) -> R {
        if accepted {
            accept(&self.inner)
        } else {
            reject()
        }
    }
}

/// Whether `key`'s label set matches `expected` exactly.
///
/// Mirrors the exact-match assertions in [`super::tests`] so production and
/// tests share one label vocabulary.
fn exact_labels(key: &Key, expected: &[(&str, &[&str])]) -> bool {
    let labels: Vec<_> = key.labels().collect();
    labels.len() == expected.len()
        && labels
            .iter()
            .zip(expected)
            .all(|(label, &(name, values))| label.key() == name && values.contains(&label.value()))
}

impl metrics::Recorder for ConfigMetricsRecorder {
    fn describe_counter(&self, key_name: KeyName, unit: Option<Unit>, description: SharedString) {
        self.forward(
            Self::accepts_name(key_name.as_str()),
            || {},
            |inner| inner.describe_counter(key_name, unit, description),
        );
    }

    fn describe_gauge(&self, key_name: KeyName, unit: Option<Unit>, description: SharedString) {
        self.forward(
            Self::accepts_name(key_name.as_str()),
            || {},
            |inner| inner.describe_gauge(key_name, unit, description),
        );
    }

    fn describe_histogram(&self, key_name: KeyName, unit: Option<Unit>, description: SharedString) {
        self.forward(
            Self::accepts_name(key_name.as_str()),
            || {},
            |inner| inner.describe_histogram(key_name, unit, description),
        );
    }

    fn register_counter(&self, key: &Key, metadata: &Metadata<'_>) -> Counter {
        self.forward(
            Self::accepts_registration(key, MetricKind::Counter),
            Counter::noop,
            |inner| inner.register_counter(key, metadata),
        )
    }

    fn register_gauge(&self, key: &Key, metadata: &Metadata<'_>) -> Gauge {
        self.forward(
            Self::accepts_registration(key, MetricKind::Gauge),
            Gauge::noop,
            |inner| inner.register_gauge(key, metadata),
        )
    }

    fn register_histogram(&self, key: &Key, metadata: &Metadata<'_>) -> Histogram {
        self.forward(
            Self::accepts_registration(key, MetricKind::Histogram),
            Histogram::noop,
            |inner| inner.register_histogram(key, metadata),
        )
    }
}
