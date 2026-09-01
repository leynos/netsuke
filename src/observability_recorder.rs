//! Bounded process recorder for configuration observability.

use super::{
    CONFIG_LOAD_COUNTER, CONFIG_LOAD_DURATION, DIAG_MODE_PHASE, MERGE_PHASE,
    STARTUP_CONFIG_LOAD_COUNTER, STARTUP_CONFIG_LOAD_DURATION,
};
use metrics::{Counter, Gauge, Histogram, Key, KeyName, Metadata, SharedString, Unit};
use metrics_util::MetricKind;
use metrics_util::debugging::{DebuggingRecorder, Snapshotter};

use netsuke::{
    cli::{DISCOVERY_DURATION, DISCOVERY_OUTCOME_VALUES, DISCOVERY_TOTAL},
    runner::{
        BASH_PREFLIGHT_TOTAL, LEGACY_RECIPE_EXECUTION_DURATION, LEGACY_RECIPE_EXECUTIONS_TOTAL,
        RECIPE_SHELL_RESOLUTIONS_TOTAL,
    },
};

/// Counter emitted by the library for bounded timing-summary sink outcomes.
pub(super) const TIMING_SUMMARY_SINK_WRITES_TOTAL: &str =
    "netsuke_status_timing_summary_writes_total";
/// Histogram emitted by the library for synchronous timing-summary sink writes.
pub(super) const TIMING_SUMMARY_SINK_WRITE_DURATION: &str =
    "netsuke_status_timing_summary_write_duration_seconds";
/// Bounded outcomes admitted for timing-summary sink counters.
const TIMING_SUMMARY_SINK_WRITE_OUTCOMES: [&str; 2] = ["success", "write_error"];

/// Label key naming the configuration-load phase on every series.
const PHASE_LABEL: &str = "phase";
/// Label key naming the outcome on configuration-load counter series.
const OUTCOME_LABEL: &str = "outcome";
/// The bounded phase values accepted on every configuration-load series.
const PHASE_VALUES: [&str; 2] = [DIAG_MODE_PHASE, MERGE_PHASE];
/// The bounded outcome values accepted on configuration-load counter series.
const OUTCOME_VALUES: [&str; 2] = ["success", "failure"];
/// Bounded interpreter values emitted by recipe-shell counters.
const RECIPE_SHELL_VALUES: [&str; 3] = ["posix", "powershell", "bash"];
/// Bounded outcomes emitted by recipe-shell resolution.
const RECIPE_SHELL_RESOLUTION_OUTCOMES: [&str; 2] = ["success", "error"];
/// Bounded error categories emitted by recipe-shell resolution.
const RECIPE_SHELL_RESOLUTION_ERROR_CATEGORIES: [&str; 2] = ["none", "invalid_selection"];
/// Bounded outcomes emitted by Bash compatibility preflight.
const BASH_PREFLIGHT_OUTCOMES: [&str; 2] = ["success", "error"];
/// Bounded probe results emitted by Bash compatibility preflight.
const BASH_PREFLIGHT_PROBE_OUTCOMES: [&str; 4] =
    ["success", "not_found", "launch_failed", "non_zero_exit"];
/// Bounded runner-operation values emitted by legacy-recipe telemetry.
const LEGACY_RECIPE_OPERATION_VALUES: [&str; 2] = ["build", "ninja_tool"];
/// Bounded outcomes emitted by legacy-recipe operation telemetry.
const LEGACY_RECIPE_OPERATION_OUTCOMES: [&str; 2] = ["success", "error"];
/// Bounded failure categories emitted by legacy-recipe operation telemetry.
const LEGACY_RECIPE_FAILURE_CATEGORIES: [&str; 6] = [
    "none",
    "manifest",
    "graph",
    "ninja_generation",
    "ninja_io",
    "other",
];

/// Application recorder that retains only bounded observability series.
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
                | DISCOVERY_TOTAL
                | DISCOVERY_DURATION
                | TIMING_SUMMARY_SINK_WRITES_TOTAL
                | TIMING_SUMMARY_SINK_WRITE_DURATION
                | RECIPE_SHELL_RESOLUTIONS_TOTAL
                | BASH_PREFLIGHT_TOTAL
                | LEGACY_RECIPE_EXECUTIONS_TOTAL
                | LEGACY_RECIPE_EXECUTION_DURATION
        )
    }

    /// Admit exact bounded counter series by their registered name.
    fn accepts_counter_registration(key: &Key) -> bool {
        match key.name() {
            CONFIG_LOAD_COUNTER => exact_labels(
                key,
                &[
                    (PHASE_LABEL, &PHASE_VALUES),
                    (OUTCOME_LABEL, &OUTCOME_VALUES),
                ],
            ),
            STARTUP_CONFIG_LOAD_COUNTER => exact_labels(key, &[(OUTCOME_LABEL, &OUTCOME_VALUES)]),
            DISCOVERY_TOTAL => exact_labels(key, &[(OUTCOME_LABEL, &DISCOVERY_OUTCOME_VALUES)]),
            TIMING_SUMMARY_SINK_WRITES_TOTAL => {
                exact_labels(key, &[(OUTCOME_LABEL, &TIMING_SUMMARY_SINK_WRITE_OUTCOMES)])
            }
            RECIPE_SHELL_RESOLUTIONS_TOTAL => exact_labels(
                key,
                &[
                    ("recipe_shell", &RECIPE_SHELL_VALUES),
                    (OUTCOME_LABEL, &RECIPE_SHELL_RESOLUTION_OUTCOMES),
                    ("error_category", &RECIPE_SHELL_RESOLUTION_ERROR_CATEGORIES),
                ],
            ),
            BASH_PREFLIGHT_TOTAL => exact_labels(
                key,
                &[
                    ("recipe_shell", &["bash"]),
                    (OUTCOME_LABEL, &BASH_PREFLIGHT_OUTCOMES),
                    ("probe_outcome", &BASH_PREFLIGHT_PROBE_OUTCOMES),
                ],
            ),
            LEGACY_RECIPE_EXECUTIONS_TOTAL => exact_labels(
                key,
                &[
                    ("operation", &LEGACY_RECIPE_OPERATION_VALUES),
                    ("recipe_shell", &RECIPE_SHELL_VALUES),
                    (OUTCOME_LABEL, &LEGACY_RECIPE_OPERATION_OUTCOMES),
                    ("failure_category", &LEGACY_RECIPE_FAILURE_CATEGORIES),
                ],
            ),
            _ => false,
        }
    }

    /// Admit exact bounded histogram series by their registered name.
    fn accepts_histogram_registration(key: &Key) -> bool {
        match key.name() {
            CONFIG_LOAD_DURATION => exact_labels(key, &[(PHASE_LABEL, &PHASE_VALUES)]),
            STARTUP_CONFIG_LOAD_DURATION
            | DISCOVERY_DURATION
            | TIMING_SUMMARY_SINK_WRITE_DURATION => exact_labels(key, &[]),
            LEGACY_RECIPE_EXECUTION_DURATION => exact_labels(
                key,
                &[
                    ("operation", &LEGACY_RECIPE_OPERATION_VALUES),
                    ("recipe_shell", &RECIPE_SHELL_VALUES),
                    (OUTCOME_LABEL, &LEGACY_RECIPE_OPERATION_OUTCOMES),
                    ("failure_category", &LEGACY_RECIPE_FAILURE_CATEGORIES),
                ],
            ),
            _ => false,
        }
    }

    /// Admit only the exact bounded series expected of `kind`.
    ///
    /// Rejects gauges (neither vocabulary name is a gauge) and same-name
    /// series whose label set is missing, extra, or unbounded.
    fn accepts_registration(key: &Key, kind: MetricKind) -> bool {
        match kind {
            MetricKind::Counter => Self::accepts_counter_registration(key),
            MetricKind::Histogram => Self::accepts_histogram_registration(key),
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
