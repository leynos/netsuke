//! Tests for bounded `netsuke check` telemetry.

use std::time::Duration;

use anyhow::{Result, anyhow, ensure};
use metrics_util::MetricKind;
use metrics_util::debugging::{DebugValue, DebuggingRecorder};
use monotony::test_util::FixedMonotonicClock;

use super::{CHECK_DURATION, CHECK_TOTAL, CheckFailure, instrument_check};

/// The duration the injected clock reports for every recorded check.
const CHECK_ELAPSED: Duration = Duration::from_millis(1_250);

/// Record one check outcome and return its complete metrics snapshot.
fn recorded(check: impl FnOnce() -> Result<(), CheckFailure>) -> (Result<()>, Snapshot) {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let clock = FixedMonotonicClock::with_elapsed(CHECK_ELAPSED);
    let result = metrics::with_local_recorder(&recorder, || instrument_check(&clock, check));
    (result, snapshotter.snapshot().into_vec())
}

/// One drained debugging-recorder snapshot.
type Snapshot = Vec<(
    metrics_util::CompositeKey,
    Option<metrics::Unit>,
    Option<metrics::SharedString>,
    DebugValue,
)>;

/// Assert one counter and duration sample carry exactly the fixed outcome.
///
/// The duration sample must equal the injected clock's elapsed time, which
/// proves the histogram reads the clock rather than the wall clock.
fn assert_outcome(snapshot: &Snapshot, outcome: &str) -> Result<()> {
    let has_outcome = |key: &metrics_util::CompositeKey, kind, name| {
        key.kind() == kind
            && key.key().name() == name
            && key.key().labels().count() == 1
            && key
                .key()
                .labels()
                .any(|label| label.key() == "outcome" && label.value() == outcome)
    };
    ensure!(
        snapshot.iter().any(|(key, _, _, value)| {
            has_outcome(key, MetricKind::Counter, CHECK_TOTAL)
                && matches!(value, DebugValue::Counter(1))
        }),
        "the counter should record {outcome:?}: {snapshot:?}"
    );
    ensure!(
        snapshot.iter().any(|(key, _, _, value)| {
            has_outcome(key, MetricKind::Histogram, CHECK_DURATION)
                && matches!(
                    value,
                    DebugValue::Histogram(samples)
                        if samples.iter().map(|sample| sample.0).eq([CHECK_ELAPSED.as_secs_f64()])
                )
        }),
        "the duration should record {CHECK_ELAPSED:?} for {outcome:?}: {snapshot:?}"
    );
    Ok(())
}

/// Preserve every command outcome as a bounded metric label.
#[test]
fn check_records_every_fixed_outcome() -> Result<()> {
    let cases = [
        ("success", Ok(())),
        (
            "policy_failure",
            Err(CheckFailure::Policy(anyhow!("policy"))),
        ),
        (
            "analysis_failure",
            Err(CheckFailure::Analysis(anyhow!("analysis"))),
        ),
        (
            "output_failure",
            Err(CheckFailure::Output(anyhow!("output"))),
        ),
        (
            "threshold_failure",
            Err(CheckFailure::Threshold(anyhow!("threshold"))),
        ),
    ];
    for (outcome, result) in cases {
        let (actual, snapshot) = recorded(|| result);
        if outcome == "success" {
            actual?;
        } else {
            ensure!(actual.is_err(), "{outcome} should remain an error");
        }
        assert_outcome(&snapshot, outcome)?;
    }
    Ok(())
}
