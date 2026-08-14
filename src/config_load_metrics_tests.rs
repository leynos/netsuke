//! Tests for configuration-load metric recording during startup.
//!
//! This module captures metrics emitted by the `run_with_args` startup boundary
//! and validates the configuration-load metrics contract for JSON-resolution
//! failures, merge failures, and successful merges.

use super::*;
use anyhow::{Result, bail, ensure};
use cap_std::ambient_authority;
use cap_std::fs::Dir;
use metrics::Label;
use metrics_util::debugging::{DebugValue, DebuggingRecorder};
use proptest::{prelude::*, test_runner::TestCaseError};
use rstest::rstest;
use std::ffi::OsString;
use std::process::ExitCode;
use std::time::Duration;
use tempfile::tempdir;

/// Capture the metrics emitted while `body` runs against a local recorder.
fn captured_metrics(body: impl FnOnce()) -> Vec<(String, Vec<Label>, DebugValue)> {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, body);
    snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .map(|(composite, _unit, _description, value)| {
            let key = composite.key();
            (
                key.name().to_owned(),
                key.labels().cloned().collect::<Vec<_>>(),
                value,
            )
        })
        .collect()
}

/// Verify the configuration-load metric contract for one startup path.
fn verify_config_load_metrics(
    metrics: &[(String, Vec<Label>, DebugValue)],
    scenario: ConfigurationLoadScenario,
    expected_duration: f64,
) -> Result<()> {
    verify_startup_outcome_counter(metrics, scenario.expected_outcome())?;
    verify_startup_duration_histogram(metrics, expected_duration)?;
    verify_phase_metrics(metrics, scenario.expected_phase_outcomes())?;
    Ok(())
}

/// Verify the public startup-attempt counter for one configuration path.
fn verify_startup_outcome_counter(
    metrics: &[(String, Vec<Label>, DebugValue)],
    expected_outcome: &str,
) -> Result<()> {
    let counters = metrics
        .iter()
        .filter(|(name, _, _)| name == "netsuke_config_load_total")
        .collect::<Vec<_>>();
    ensure!(
        counters.len() == 1,
        "one config-load counter should be recorded, found {}",
        counters.len()
    );
    let [counter] = counters.as_slice() else {
        bail!("one config-load counter should be recorded");
    };
    let [label] = counter.1.as_slice() else {
        bail!("counter should have exactly one label");
    };
    ensure!(
        label.key() == "outcome",
        "counter label key must be outcome"
    );
    ensure!(
        label.value() == expected_outcome,
        "counter outcome must be {expected_outcome}"
    );
    ensure!(
        counter.2 == DebugValue::Counter(1),
        "counter must record exactly one attempt"
    );
    Ok(())
}

/// Verify the public startup-attempt duration histogram for one configuration path.
fn verify_startup_duration_histogram(
    metrics: &[(String, Vec<Label>, DebugValue)],
    expected_duration: f64,
) -> Result<()> {
    let histograms = metrics
        .iter()
        .filter(|(name, _, _)| name == "netsuke_config_load_duration_seconds")
        .collect::<Vec<_>>();
    ensure!(
        histograms.len() == 1,
        "one config-load histogram should be recorded, found {}",
        histograms.len()
    );
    let [histogram] = histograms.as_slice() else {
        bail!("one config-load histogram should be recorded");
    };
    let DebugValue::Histogram(samples) = &histogram.2 else {
        bail!("expected a histogram value, got {:?}", histogram.2);
    };
    ensure!(histogram.1.is_empty(), "histogram must have no labels");
    ensure!(samples.len() == 1, "exactly one duration sample expected");
    let [sample] = samples.as_slice() else {
        bail!("exactly one duration sample expected");
    };
    ensure!(
        *sample == expected_duration,
        "duration sample must be {expected_duration}"
    );
    Ok(())
}

/// Verify the phase-level metric records from the startup configuration path.
fn verify_phase_metrics(
    metrics: &[(String, Vec<Label>, DebugValue)],
    expected_phase_outcomes: &[(&str, &str)],
) -> Result<()> {
    let counters = metrics
        .iter()
        .filter(|(name, _, _)| name == "config_load_total")
        .collect::<Vec<_>>();
    ensure!(
        counters.len() == expected_phase_outcomes.len(),
        "expected one phase counter per configuration path"
    );
    let histograms = metrics
        .iter()
        .filter(|(name, _, _)| name == "config_load_duration_seconds")
        .collect::<Vec<_>>();
    ensure!(
        histograms.len() == expected_phase_outcomes.len(),
        "expected one phase histogram per configuration path"
    );

    for (phase, outcome) in expected_phase_outcomes {
        let counter_count = counters
            .iter()
            .filter(|counter| {
                has_exact_labels(&counter.1, &[("phase", phase), ("outcome", outcome)])
                    && counter.2 == DebugValue::Counter(1)
            })
            .count();
        ensure!(
            counter_count == 1,
            "expected one phase counter for phase={phase}, outcome={outcome}"
        );
        let histogram_count = histograms
            .iter()
            .filter(|histogram| {
                has_exact_labels(&histogram.1, &[("phase", phase)])
                    && matches!(&histogram.2, DebugValue::Histogram(samples) if samples.len() == 1)
            })
            .count();
        ensure!(
            histogram_count == 1,
            "expected one phase histogram for phase={phase}"
        );
    }
    Ok(())
}

/// Check that a metric has all and only the expected bounded labels.
fn has_exact_labels(labels: &[Label], expected: &[(&str, &str)]) -> bool {
    labels.len() == expected.len()
        && expected.iter().all(|(key, value)| {
            labels
                .iter()
                .any(|label| label.key() == *key && label.value() == *value)
        })
}

struct EmptyEnv;

impl locale_resolution::LocaleEnvProvider for EmptyEnv {
    fn var(&self, _key: &str) -> Option<String> {
        None
    }
}

struct NoSystemLocale;

impl locale_resolution::SystemLocale for NoSystemLocale {
    fn system_locale(&self) -> Option<String> {
        None
    }
}

/// A deterministic elapsed-time source for startup-boundary tests.
struct FixedConfigurationLoadClock(Duration);

impl config_load::ConfigurationLoadClock for FixedConfigurationLoadClock {
    fn restart(&mut self) {}

    fn elapsed(&self) -> Duration {
        self.0
    }
}

/// One real configuration-load route and its expected metric outcome.
#[derive(Clone, Copy)]
enum ConfigurationLoadScenario {
    JsonResolutionFailure,
    MergeFailure,
    SuccessfulMerge,
}

impl ConfigurationLoadScenario {
    const ALL: [Self; 3] = [
        Self::JsonResolutionFailure,
        Self::MergeFailure,
        Self::SuccessfulMerge,
    ];

    const fn config_contents(self) -> Option<&'static str> {
        match self {
            Self::JsonResolutionFailure => None,
            Self::MergeFailure => Some("jobs = 0\n"),
            Self::SuccessfulMerge => Some(""),
        }
    }

    const fn expected_outcome(self) -> &'static str {
        match self {
            Self::JsonResolutionFailure | Self::MergeFailure => "failure",
            Self::SuccessfulMerge => "success",
        }
    }

    const fn expected_phase_outcomes(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::JsonResolutionFailure => &[("diag_mode", "failure")],
            Self::MergeFailure => &[("diag_mode", "success"), ("merge", "failure")],
            Self::SuccessfulMerge => &[("diag_mode", "success"), ("merge", "success")],
        }
    }
}

/// Run a configuration scenario through the binary orchestration and capture
/// its configuration metrics.
fn run_with_config_metrics(
    scenario: ConfigurationLoadScenario,
    duration: Duration,
) -> Result<Vec<(String, Vec<Label>, DebugValue)>> {
    let temp = tempdir()?;
    let config_path = temp.path().join("netsuke.toml");
    if let Some(contents) = scenario.config_contents() {
        Dir::open_ambient_dir(temp.path(), ambient_authority())?.write("netsuke.toml", contents)?;
    }
    let missing_manifest = temp.path().join("missing.Netsukefile");
    let args: Vec<OsString> = [
        "netsuke".into(),
        "--config".into(),
        config_path.into_os_string(),
        "--file".into(),
        missing_manifest.into_os_string(),
        "--json".into(),
    ]
    .into();
    let _lock = test_support::localizer_test_lock()
        .map_err(|error| anyhow::anyhow!("localizer test lock poisoned: {error}"))?;
    let _restore = localization::set_localizer_for_tests(localization::localizer());
    let mut clock = FixedConfigurationLoadClock(duration);
    let mut exit = None;
    let metrics = captured_metrics(|| {
        exit = Some(run_with_args(args, &EmptyEnv, &NoSystemLocale, &mut clock));
    });
    ensure!(
        exit == Some(ExitCode::FAILURE),
        "configuration scenario should fail after recording its metric"
    );
    Ok(metrics)
}

/// Each configuration-resolution exit records exactly one failure outcome.
#[rstest]
#[case::json_resolution_failure(ConfigurationLoadScenario::JsonResolutionFailure)]
#[case::merge_failure(ConfigurationLoadScenario::MergeFailure)]
fn configuration_failures_record_metrics_for_their_exit_path(
    #[case] scenario: ConfigurationLoadScenario,
) -> Result<()> {
    let duration = Duration::from_millis(7);
    let metrics = run_with_config_metrics(scenario, duration)?;

    verify_config_load_metrics(&metrics, scenario, duration.as_secs_f64())?;
    Ok(())
}

/// A successfully merged configuration records success before runner failure.
#[test]
fn a_successful_configuration_merge_records_metrics_before_runner_failure() -> Result<()> {
    let duration = Duration::from_millis(11);
    let scenario = ConfigurationLoadScenario::SuccessfulMerge;
    let metrics = run_with_config_metrics(scenario, duration)?;

    verify_config_load_metrics(&metrics, scenario, duration.as_secs_f64())?;
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 8, .. ProptestConfig::default() })]

    /// Every modelled startup outcome records the same bounded metric contract.
    #[test]
    fn configuration_load_outcomes_preserve_the_metrics_contract(milliseconds in 0_u64..=1_000) {
        let duration = Duration::from_millis(milliseconds);
        for scenario in ConfigurationLoadScenario::ALL {
            let metrics = run_with_config_metrics(scenario, duration)
                .map_err(|error| TestCaseError::fail(error.to_string()))?;
            verify_config_load_metrics(&metrics, scenario, duration.as_secs_f64())
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        }
    }
}
