//! Check telemetry boundaries around runner-owned Ninja builds.

use super::*;
use crate::runner::{NINJA_BUILD_EXECUTIONS_TOTAL, run_with_ninja_program};
use anyhow::{Result, ensure};
use metrics_util::debugging::DebuggingRecorder;
use rstest::rstest;
use test_support::{localizer_test_lock, set_en_localizer};

#[rstest]
#[case::progress_enabled(crate::cli::ProgressPolicy::Auto)]
#[case::progress_disabled(crate::cli::ProgressPolicy::Never)]
fn invalid_job_options_do_not_record_a_ninja_build(
    #[case] progress: crate::cli::ProgressPolicy,
) -> Result<()> {
    let _lock = localizer_test_lock().map_err(|error| anyhow::anyhow!("{error}"))?;
    let _guard = set_en_localizer();
    let (_temp, manifest_path) = write_manifest(MINIMAL_MANIFEST)?;
    let cli = Cli {
        file: manifest_path,
        jobs: Some(0),
        progress,
        ..Cli::default()
    };
    let program = camino::Utf8Path::new("ninja");
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    let (result, metrics) = metrics::with_local_recorder(&recorder, || {
        let result = run_with_ninja_program(&cli, crate::output_prefs::resolve(None), program);
        (result, snapshotter.snapshot().into_vec())
    });

    ensure!(
        result.is_err(),
        "an invalid job count must fail before Ninja runs"
    );
    ensure!(
        !metrics
            .iter()
            .any(|(key, _, _, _)| key.key().name() == NINJA_BUILD_EXECUTIONS_TOTAL),
        "option validation must not record a Ninja build: {metrics:?}"
    );
    Ok(())
}
