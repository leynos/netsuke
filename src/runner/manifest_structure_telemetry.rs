//! Bounded structural telemetry for the rendered manifest.
//!
//! This module records only fixed-vocabulary aggregate counts derived from the
//! loaded manifest shape (variables, macros, rules, actions, targets, and
//! defaults). It never emits manifest text, paths, recipe contents,
//! variable values, or descriptions, because rendered manifest values can
//! carry secret material derived from `env()` interpolation.

use crate::ast::NetsukeManifest;
use metrics::{counter, describe_counter};
use std::sync::Once;
use tracing::{trace, trace_span};

/// Metric counting manifest structural summaries emitted by the runner.
const MANIFEST_STRUCTURES_TOTAL: &str = "netsuke_runner_manifest_structures_total";

/// Describe the manifest structure counter once per process.
fn describe_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            MANIFEST_STRUCTURES_TOTAL,
            "Counts runner-owned manifest structural summaries by collection sizes."
        );
    });
}

/// Emit a bounded structural summary of the rendered manifest.
///
/// The summary is emitted at `TRACE` level using fixed-vocabulary integer
/// fields only, so no manifest text can cross the telemetry boundary.
///
/// # Examples
///
/// ```ignore
/// # use netsuke::ast::NetsukeManifest;
/// # use netsuke::runner::manifest_structure_telemetry::record_manifest_structure;
/// let manifest = NetsukeManifest::default();
/// record_manifest_structure(&manifest);
/// ```
///
/// # Panics
///
/// Panics if the global metrics recorder is not installed. Callers must
/// initialise the metrics pipeline before invoking this function.
pub fn record_manifest_structure(manifest: &NetsukeManifest) {
    describe_metrics();
    let span = trace_span!(
        "runner.manifest.structure",
        variable_count = manifest.vars.len(),
        macro_count = manifest.macros.len(),
        rule_count = manifest.rules.len(),
        action_count = manifest.actions.len(),
        target_count = manifest.targets.len(),
        default_count = manifest.defaults.len(),
    );
    let _guard = span.enter();
    trace!(
        variable_count = manifest.vars.len(),
        macro_count = manifest.macros.len(),
        rule_count = manifest.rules.len(),
        action_count = manifest.actions.len(),
        target_count = manifest.targets.len(),
        default_count = manifest.defaults.len(),
        "manifest structure summary"
    );
    counter!(MANIFEST_STRUCTURES_TOTAL).increment(1);
}

#[cfg(test)]
mod tests {
    //! Tests for bounded manifest-structure telemetry at the runner boundary.

    use super::*;
    use crate::test_tracing_capture::with_test_subscriber;
    use anyhow::{Context, Result, ensure};
    use metrics_util::{
        MetricKind,
        debugging::{DebugValue, DebuggingRecorder},
    };
    use tracing_subscriber::filter::LevelFilter;

    /// Distinctive value standing in for a recipe value rendered from `env()`.
    const SENTINEL: &str = "CI-SECRET-7f3a9c2b5e1d4f60";

    /// Build a manifest whose collection sizes and text are both known.
    ///
    /// Every field that can carry text holds [`SENTINEL`], so a telemetry field
    /// that leaked manifest text instead of a count would be observable. The
    /// fixture is parsed before a capture subscriber is installed, keeping the
    /// parser's own trace events out of the captured field set. Parsing returns
    /// its error rather than expecting, because the workspace lint suite allows
    /// `expect` only inside a test body.
    fn manifest_with_known_shape() -> Result<NetsukeManifest> {
        let yaml = format!(
            r#"netsuke_version: "1.0.0"
vars:
  token: "{SENTINEL}"
  greeting: "{SENTINEL}"
macros:
  - signature: "greet(name)"
    body: "echo {SENTINEL}"
rules:
  - name: greet-rule
    command: echo {SENTINEL}
    description: "{SENTINEL}"
actions:
  - name: setup
    command: echo {SENTINEL}
targets:
  - name: app
    rule: greet-rule
    description: "{SENTINEL}"
  - name: report
    rule: greet-rule
defaults:
  - app
  - report
"#
        );
        crate::manifest::from_str(&yaml).context("the telemetry fixture manifest should parse")
    }

    /// Verify the emission site records one unlabelled manifest-structure count.
    #[test]
    fn manifest_structure_records_one_unlabelled_counter() -> Result<()> {
        let manifest = manifest_with_known_shape()?;
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();

        metrics::with_local_recorder(&recorder, || {
            record_manifest_structure(&manifest);
        });

        let snapshot = snapshotter.snapshot().into_vec();
        ensure!(
            snapshot.len() == 1,
            "a structural summary must record exactly one metric series: {snapshot:?}"
        );
        ensure!(
            snapshot.iter().any(|entry| {
                entry.0.kind() == MetricKind::Counter
                    && entry.0.key().name() == MANIFEST_STRUCTURES_TOTAL
                    && entry.0.key().labels().next().is_none()
                    && matches!(entry.3, DebugValue::Counter(1))
            }),
            "expected the unlabelled {MANIFEST_STRUCTURES_TOTAL} counter: {snapshot:?}"
        );
        Ok(())
    }

    /// Verify the trace event carries only fixed-vocabulary collection sizes.
    #[test]
    fn manifest_structure_telemetry_carries_only_counts() -> Result<()> {
        let manifest = manifest_with_known_shape()?;
        let recorder = DebuggingRecorder::new();

        let fields = with_test_subscriber(LevelFilter::TRACE, |captured| {
            metrics::with_local_recorder(&recorder, || {
                record_manifest_structure(&manifest);
            });
            captured.snapshot()
        });

        ensure!(
            fields.len() == 1,
            "a structural summary must emit exactly one trace event: {fields:?}"
        );
        let event = fields
            .first()
            .context("the event field set was asserted above")?;
        for expected in [
            "message=manifest structure summary",
            "variable_count=2",
            "macro_count=1",
            "rule_count=1",
            "action_count=1",
            "target_count=2",
            "default_count=2",
        ] {
            ensure!(
                event.contains(expected),
                "trace event should report {expected:?}, got {event:?}"
            );
        }
        ensure!(
            !event.contains(SENTINEL),
            "trace event must not carry rendered manifest values, got {event:?}"
        );
        Ok(())
    }
}
