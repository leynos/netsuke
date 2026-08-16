//! Telemetry-contract tests for atomic dyndep sidecar materialization.

use super::*;

#[test]
fn materialization_records_success_metrics() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temp_dir(&temp)?;
    let dyndep = sidecar(".netsuke/dyndep/metrics.dd", "content");

    let (result, snapshot) = recorded(|| materialize_dyndep_files(&dir, &[dyndep]));

    result?;
    assert_materialization_metrics(&snapshot, "success")
}

#[test]
fn materialization_records_error_metrics() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temp_dir(&temp)?;
    let rel = ".netsuke/dyndep/metrics-error.dd";
    dir.create_dir_all(DYNDEP_DIR)?;
    dir.write(rel, "corrupt")?;

    let (result, snapshot) =
        recorded(|| materialize_dyndep_files(&dir, &[sidecar(rel, "expected")]));

    ensure!(result.is_err(), "corrupt sidecar must fail materialization");
    assert_materialization_metrics(&snapshot, "error")
}
