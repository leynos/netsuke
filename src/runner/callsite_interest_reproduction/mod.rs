//! Fresh-process reproduction for stale manifest telemetry callsite interest.
//!
//! The ignored worker gates the first summary-event registration while a
//! second thread installs a TRACE subscriber, then emits the same production
//! callsites after T1 finishes. Running in a fresh test process ensures no
//! earlier test has registered those callsites.

mod driver;
mod gate;
mod threads;

use super::manifest_structure_telemetry::validate_manifest_structure_events;
use anyhow::{Context, Result, anyhow, ensure};
use mockable::Env;
use std::time::Duration;
use threads::{CaptureMode, CaptureSnapshot, run_worker_threads};

const MODE_ENV: &str = "NETSUKE_ISSUE_927_WORKER_MODE";
const CHILD_DEADLINE: Duration = Duration::from_secs(20);
const EMPTY_MANIFEST_COUNTS: [(&str, usize); 6] = [
    ("variable_count", 0),
    ("macro_count", 0),
    ("rule_count", 0),
    ("action_count", 0),
    ("target_count", 0),
    ("default_count", 0),
];

/// Emit the ignored worker schedule selected through its child-only environment.
#[test]
#[ignore = "driven in a fresh process by the issue 927 parent test"]
fn interest_worker() -> Result<()> {
    let mode = mockable::DefaultEnv
        .os_string(MODE_ENV)
        .context("read the child-only schedule mode")?
        .into_string()
        .map_err(|mode| anyhow!("child-only schedule mode must be UTF-8: {mode:?}"))?;
    match mode.as_str() {
        "unmitigated" => run_unmitigated_worker(),
        "mitigated" => run_mitigated_worker(),
        _ => Err(anyhow!("unexpected issue 927 worker mode {mode:?}")),
    }
}

/// Prove the original stale-interest schedule still fails without anchors.
fn run_unmitigated_worker() -> Result<()> {
    let captured = run_worker_threads(CaptureMode::Unmitigated)?;
    validate_unmitigated_capture(&captured)?;
    print_capture_markers(&captured, "installed", "zero-rejected")
}

/// Prove anchors prevent the stale-interest schedule from dropping the event.
fn run_mitigated_worker() -> Result<()> {
    test_support::tracing_capture::ensure_test_interest_anchors_registered();
    let captured = run_worker_threads(CaptureMode::Mitigated)?;
    validate_mitigated_capture(&captured)?;
    print_capture_markers(&captured, "deadline", "one-accepted")
}

/// Require the unmitigated schedule to retain the span and lose the event.
fn validate_unmitigated_capture(captured: &CaptureSnapshot) -> Result<()> {
    ensure!(
        captured.span_fields.len() == 6,
        "the independent structural span callsite must be captured: {:?}",
        captured.span_fields
    );
    for (name, value) in EMPTY_MANIFEST_COUNTS {
        let expected = format!("{name}={value}");
        ensure!(
            captured
                .span_fields
                .iter()
                .filter(|field| **field == expected)
                .count()
                == 1,
            "the structural span must carry {expected:?}: {:?}",
            captured.span_fields
        );
    }
    ensure!(
        captured.events.is_empty(),
        "the unmitigated canary no longer loses the summary event: {:?}; upstream may have fixed the race and the test anchors may be removable",
        captured.events,
    );
    ensure!(
        validate_manifest_structure_events(&captured.events, &EMPTY_MANIFEST_COUNTS).is_err(),
        "the exact-cardinality oracle must reject the reproduced empty capture"
    );
    Ok(())
}

/// Require the anchored capture to satisfy the exact event oracle.
fn validate_mitigated_capture(captured: &CaptureSnapshot) -> Result<()> {
    ensure!(
        captured.span_fields.len() == 6,
        "the independent structural span callsite must be captured: {:?}",
        captured.span_fields
    );
    validate_manifest_structure_events(&captured.events, &EMPTY_MANIFEST_COUNTS).map_err(|error| {
        anyhow!(
            "anchored capture failed its exact-cardinality oracle: {error}; events: {:?}",
            captured.events
        )
    })
}

/// Print the release, span, event, and oracle markers required by the parent.
fn print_capture_markers(
    captured: &CaptureSnapshot,
    release_cause: &str,
    oracle: &str,
) -> Result<()> {
    write_marker("ISSUE927_GATE_PAUSED=true")?;
    write_marker("ISSUE927_REGISTRATION_ATTEMPT=true")?;
    write_marker(&format!("ISSUE927_RELEASE_CAUSE={release_cause}"))?;
    write_marker(&format!("ISSUE927_SPAN_FIELDS={:?}", captured.span_fields))?;
    write_marker(&format!("ISSUE927_EVENT_SNAPSHOT={:?}", captured.events))?;
    write_marker(&format!("ISSUE927_ORACLE={oracle}"))?;
    Ok(())
}

/// Write one required phase marker to child stdout.
fn write_marker(marker: &str) -> std::io::Result<()> {
    use std::io::Write;

    let mut output = std::io::stdout().lock();
    writeln!(output, "{marker}")?;
    output.flush()
}
