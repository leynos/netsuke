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
const SENTINEL: &str = "CI-SECRET-7f3a9c2b5e1d4f60";

/// Require one structural-summary event with the expected count fields.
///
/// The event message is matched as a phrase because tracing renders its words
/// with spaces, while each count must remain one exact `name=value` token.
///
/// # Errors
///
/// Returns an error if the summary is absent or duplicated, any count token is
/// absent or duplicated, or captured event data contains [`SENTINEL`].
#[cfg(test)]
pub(crate) fn validate_manifest_structure_events(
    events: &[String],
    expected_counts: &[(&str, usize)],
) -> anyhow::Result<()> {
    use anyhow::{Context, ensure};

    ensure!(
        events.iter().all(|event| !event.contains(SENTINEL)),
        "trace events must not contain rendered manifest values: {events:?}"
    );
    let summaries = events
        .iter()
        .filter(|event| is_manifest_structure_summary(event))
        .collect::<Vec<_>>();
    ensure!(
        summaries.len() == 1,
        "a structural summary must emit exactly one trace event: {events:?}"
    );
    let summary = summaries
        .first()
        .context("the exact-cardinality check found no summary")?;
    let tokens = summary.split_whitespace().collect::<Vec<_>>();
    ensure!(
        tokens.len() == expected_counts.len() + 3,
        "a structural summary must contain its message and six count fields only: {summary:?}"
    );
    validate_count_tokens(&tokens, expected_counts)?;
    ensure!(
        tokens.iter().filter(|token| token.contains('=')).count() == expected_counts.len() + 1,
        "a structural summary must contain only its message and count fields: {summary:?}"
    );
    Ok(())
}

/// Match the exact rendered message-field token sequence for the summary.
#[cfg(test)]
fn is_manifest_structure_summary(event: &str) -> bool {
    event
        .split_whitespace()
        .collect::<Vec<_>>()
        .windows(3)
        .any(|tokens| tokens == ["message=manifest", "structure", "summary"])
}

/// Require each expected count token to appear exactly once.
#[cfg(test)]
fn validate_count_tokens(tokens: &[&str], expected_counts: &[(&str, usize)]) -> anyhow::Result<()> {
    use anyhow::ensure;

    for (name, count) in expected_counts {
        let expected = format!("{name}={count}");
        let matches = tokens.iter().filter(|token| **token == expected).count();
        ensure!(
            matches == 1,
            "trace fields must contain {expected:?} exactly once: {tokens:?}"
        );
    }
    Ok(())
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

    /// Collection sizes for the manifest fixture used by the telemetry tests.
    const KNOWN_SHAPE_COUNTS: [(&str, usize); 6] = [
        ("variable_count", 2),
        ("macro_count", 1),
        ("rule_count", 1),
        ("action_count", 1),
        ("target_count", 2),
        ("default_count", 2),
    ];

    /// Build a synthetic summary event for exercising the exact-cardinality oracle.
    fn synthetic_summary() -> String {
        "variable_count=2 macro_count=1 rule_count=1 action_count=1 target_count=2 default_count=2 message=manifest structure summary".to_owned()
    }

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

        let (events, span_fields) = with_test_subscriber(LevelFilter::TRACE, |captured| {
            metrics::with_local_recorder(&recorder, || {
                record_manifest_structure(&manifest);
            });
            (
                captured.snapshot(),
                captured.span_fields("runner.manifest.structure"),
            )
        });

        validate_manifest_structure_events(&events, &KNOWN_SHAPE_COUNTS)?;
        let span_tokens = span_fields.iter().map(String::as_str).collect::<Vec<_>>();
        ensure!(
            span_tokens.len() == KNOWN_SHAPE_COUNTS.len(),
            "the structural span must carry exactly six count fields: {span_fields:?}"
        );
        validate_count_tokens(&span_tokens, &KNOWN_SHAPE_COUNTS)?;
        Ok(())
    }

    /// Reject zero and duplicate summary events while accepting one exact event.
    #[test]
    fn summary_validator_has_zero_one_and_duplicate_controls() -> Result<()> {
        let summary = synthetic_summary();
        let one = vec![summary.clone()];
        validate_manifest_structure_events(&one, &KNOWN_SHAPE_COUNTS)?;
        ensure!(
            validate_manifest_structure_events(&[], &KNOWN_SHAPE_COUNTS).is_err(),
            "the validator must reject an empty capture"
        );
        ensure!(
            validate_manifest_structure_events(
                &[summary.clone(), summary.clone()],
                &KNOWN_SHAPE_COUNTS
            )
            .is_err(),
            "the validator must reject duplicate summaries"
        );
        let seeded_secret = vec![format!("{summary} {SENTINEL}")];
        ensure!(
            validate_manifest_structure_events(&seeded_secret, &KNOWN_SHAPE_COUNTS).is_err(),
            "the validator must reject rendered manifest values"
        );
        Ok(())
    }

    /// Reject a message-like substring that is not the exact summary field.
    #[test]
    fn summary_validator_requires_the_exact_message_field() -> Result<()> {
        let summary = synthetic_summary().replace(
            "message=manifest structure summary",
            "prefix=message=manifest structure summary",
        );
        ensure!(
            validate_manifest_structure_events(&[summary], &KNOWN_SHAPE_COUNTS).is_err(),
            "the validator must reject a summary phrase embedded in another field"
        );
        Ok(())
    }

    /// Reject two summaries emitted through the real TRACE callsite.
    #[test]
    fn summary_validator_rejects_a_seeded_duplicate_trace_event() -> Result<()> {
        let manifest = manifest_with_known_shape()?;
        let recorder = DebuggingRecorder::new();
        let events = with_test_subscriber(LevelFilter::TRACE, |captured| {
            metrics::with_local_recorder(&recorder, || {
                record_manifest_structure(&manifest);
                record_manifest_structure(&manifest);
            });
            captured.snapshot()
        });
        ensure!(
            events.len() == 2,
            "the seeded duplicate control must capture two real events: {events:?}"
        );
        ensure!(
            validate_manifest_structure_events(&events, &KNOWN_SHAPE_COUNTS).is_err(),
            "the exact-cardinality validator must reject the real duplicate"
        );
        Ok(())
    }
}
