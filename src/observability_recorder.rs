//! Bounded process recorder for configuration observability.

use super::{CONFIG_LOAD_COUNTER, CONFIG_LOAD_DURATION};
use metrics_util::debugging::{DebuggingRecorder, Snapshotter};

/// Application recorder that retains only bounded configuration-load metrics.
///
/// The process-wide debugging recorder is a shutdown-only diagnostic aid. It
/// must not retain workload-proportional observations from unrelated metrics.
#[derive(Debug)]
pub(super) struct ConfigMetricsRecorder {
    inner: DebuggingRecorder,
}

impl ConfigMetricsRecorder {
    pub(super) fn new() -> Self {
        Self {
            inner: DebuggingRecorder::new(),
        }
    }

    pub(super) fn snapshotter(&self) -> Snapshotter {
        self.inner.snapshotter()
    }

    fn accepts_name(name: &str) -> bool {
        matches!(name, CONFIG_LOAD_COUNTER | CONFIG_LOAD_DURATION)
    }
}

impl metrics::Recorder for ConfigMetricsRecorder {
    fn describe_counter(
        &self,
        key_name: metrics::KeyName,
        unit: Option<metrics::Unit>,
        description: metrics::SharedString,
    ) {
        if Self::accepts_name(key_name.as_str()) {
            self.inner.describe_counter(key_name, unit, description);
        }
    }

    fn describe_gauge(
        &self,
        key_name: metrics::KeyName,
        unit: Option<metrics::Unit>,
        description: metrics::SharedString,
    ) {
        if Self::accepts_name(key_name.as_str()) {
            self.inner.describe_gauge(key_name, unit, description);
        }
    }

    fn describe_histogram(
        &self,
        key_name: metrics::KeyName,
        unit: Option<metrics::Unit>,
        description: metrics::SharedString,
    ) {
        if Self::accepts_name(key_name.as_str()) {
            self.inner.describe_histogram(key_name, unit, description);
        }
    }

    fn register_counter(
        &self,
        key: &metrics::Key,
        metadata: &metrics::Metadata<'_>,
    ) -> metrics::Counter {
        if Self::accepts_name(key.name()) {
            self.inner.register_counter(key, metadata)
        } else {
            metrics::Counter::noop()
        }
    }

    fn register_gauge(
        &self,
        key: &metrics::Key,
        metadata: &metrics::Metadata<'_>,
    ) -> metrics::Gauge {
        if Self::accepts_name(key.name()) {
            self.inner.register_gauge(key, metadata)
        } else {
            metrics::Gauge::noop()
        }
    }

    fn register_histogram(
        &self,
        key: &metrics::Key,
        metadata: &metrics::Metadata<'_>,
    ) -> metrics::Histogram {
        if Self::accepts_name(key.name()) {
            self.inner.register_histogram(key, metadata)
        } else {
            metrics::Histogram::noop()
        }
    }
}
