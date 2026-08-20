//! Regression tests for startup configuration-resolution orchestration.

use super::*;
use crate::config_resolution::JsonModeResolutionContext;
use anyhow::{Result, anyhow, bail, ensure};
use cap_std::{ambient_authority, fs::Dir};
use clap::CommandFactory;
use metrics_util::{
    MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};
use monotony::test_util::ManualMonotonicClock;
use std::ffi::OsString;
use std::time::{Duration, Instant};
use tempfile::tempdir;

/// Deterministic empty environment for startup-resolution tests.
struct EmptyConfigEnv;

impl cli::ConfigEnvProvider for EmptyConfigEnv {
    fn get(&self, _key: &str) -> Option<OsString> {
        None
    }
}

/// A timed diagnostic resolution includes discovery and replays once.
#[test]
fn diagnostic_resolution_records_discovery_duration_and_replays_once() -> Result<()> {
    let directory = tempdir()?;
    let config_path = directory.path().join("customer@example.com.toml");
    let config_dir = Dir::open_ambient_dir(directory.path(), ambient_authority())?;
    config_dir.write("customer@example.com.toml", b"json = false\n")?;
    let cli = cli::Cli {
        config: Some(config_path),
        ..cli::Cli::default()
    };
    let matches = cli::Cli::command().get_matches_from(["netsuke"]);
    let env = EmptyConfigEnv;
    let clock = ManualMonotonicClock::new(Instant::now());
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let writer = StartupWriter::buffering();
    let subscriber = Registry::default()
        .with(LevelFilter::TRACE)
        .with(fmt::layer().with_writer(writer.clone()).with_ansi(false));

    let resolution = metrics::with_local_recorder(&recorder, || {
        tracing::subscriber::with_default(subscriber, || {
            JsonModeResolutionContext {
                parsed_cli: &cli,
                matches: &matches,
                fallback_mode: DiagMode::Human,
                env: &env,
                clock: &clock,
            }
            .resolve_with(|resolved_cli, resolved_matches, config_env| {
                clock.advance(Duration::from_millis(25));
                cli::resolve_json_and_layers_outcome_with_env(
                    resolved_cli,
                    resolved_matches,
                    config_env,
                )
            })
        })
    });

    let (mode, layers) = resolution.map_err(|_| anyhow!("diagnostic resolution should succeed"))?;
    ensure!(
        mode == DiagMode::Human,
        "config JSON preference should select human mode"
    );
    drop(layers);
    let events = String::from_utf8_lossy(&writer.buffered()).into_owned();
    ensure!(
        events.matches("using explicit config path").count() == 1,
        "deferred diagnostics should replay exactly once: {events}"
    );

    let snapshot = snapshotter.snapshot().into_vec();
    let Some(DebugValue::Histogram(samples)) = snapshot.iter().find_map(|entry| {
        (entry.0.kind() == MetricKind::Histogram
            && entry.0.key().name() == observability::CONFIG_LOAD_DURATION
            && entry.0.key().labels().count() == 1
            && entry.0.key().labels().any(|label| {
                label.key() == "phase" && label.value() == observability::DIAG_MODE_PHASE
            }))
        .then_some(&entry.3)
    }) else {
        bail!("expected a diagnostic-mode configuration duration metric: {snapshot:?}");
    };
    ensure!(
        samples.as_slice() == [0.025],
        "diagnostic resolution duration should include the resolver work: {samples:?}"
    );
    Ok(())
}
