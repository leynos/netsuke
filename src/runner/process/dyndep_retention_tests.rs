//! Focused tests for bounded dyndep sidecar retention.

use super::*;
use crate::ninja_gen::GeneratedDyndep;
use crate::runner::process::dyndep_telemetry::{
    RETAINED_BYTES_RECLAIMED, RETAINED_FILES_RECLAIMED, RETENTIONS_TOTAL,
};
use crate::runner::process::materialize_dyndep_files;
use anyhow::{Result, ensure};
use camino::Utf8PathBuf;
use cap_std::fs_utf8::Dir;
use metrics_util::{
    MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};

fn temporary_dir(temp: &tempfile::TempDir) -> Result<Dir> {
    let path = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("temporary directory is not UTF-8: {}", path.display()))?;
    Dir::open_ambient_dir(path, cap_std::ambient_authority()).map_err(Into::into)
}

fn sidecar(name: &str, content: &str) -> GeneratedDyndep {
    GeneratedDyndep::fixture(Utf8PathBuf::from(name), content.to_owned())
}

fn sidecar_names(dir: &Dir) -> Result<Vec<Utf8PathBuf>> {
    dir.read_dir(DYNDEP_DIR)?
        .map(|entry_result| {
            entry_result.and_then(|dir_entry| {
                dir_entry
                    .file_name()
                    .map(|name| Utf8Path::new(DYNDEP_DIR).join(name))
            })
        })
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn retained_sidecar_bytes(dir: &Dir) -> Result<u64> {
    sidecar_names(dir)?
        .into_iter()
        .filter(|path| has_extension(path, "dd"))
        .try_fold(0_u64, |total, path| {
            Ok(total.saturating_add(dir.metadata(path)?.len()))
        })
}

#[test]
fn retention_records_only_a_bounded_success_outcome_and_reclaimed_totals() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temporary_dir(&temp)?;
    let current = sidecar(".netsuke/dyndep/current.dd", "current");
    let lease = materialize_dyndep_files(&dir, std::slice::from_ref(&current))?;
    dir.write(".netsuke/dyndep/stale.dd", "stale")?;
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        prune_dyndep_sidecars(
            &dir,
            &lease,
            std::slice::from_ref(&current),
            RetentionPolicy::new(0, 0),
        )
    })?;

    let snapshot = snapshotter.snapshot().into_vec();
    let retention_successes = snapshot.iter().find_map(|(key, _, _, debug_value)| {
        let success = key
            .key()
            .labels()
            .any(|label| label.key() == "outcome" && label.value() == "success");
        match (key.kind(), key.key().name(), success, debug_value) {
            (MetricKind::Counter, name, true, DebugValue::Counter(count))
                if name == RETENTIONS_TOTAL =>
            {
                Some(*count)
            }
            _ => None,
        }
    });
    ensure!(
        retention_successes == Some(1),
        "retention must record exactly one fixed success outcome"
    );
    let reclaimed = |name| {
        snapshot.iter().find_map(|(key, _, _, debug_value)| {
            match (key.kind(), key.key().name(), debug_value) {
                (MetricKind::Counter, metric, DebugValue::Counter(count)) if metric == name => {
                    Some(*count)
                }
                _ => None,
            }
        })
    };
    ensure!(
        reclaimed(RETAINED_FILES_RECLAIMED) == Some(1),
        "retention must record reclaimed sidecar files"
    );
    ensure!(
        reclaimed(RETAINED_BYTES_RECLAIMED) == Some(5),
        "retention must record reclaimed sidecar bytes"
    );
    Ok(())
}

#[test]
fn repeated_publication_respects_the_obsolete_file_count_budget() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temporary_dir(&temp)?;
    let policy = RetentionPolicy::new(2, 1024);
    let mut latest_sidecar = None;

    for index in 0..6 {
        let path = format!(".netsuke/dyndep/count-{index}.dd");
        let current = sidecar(&path, &format!("content-{index}"));
        let lease = materialize_dyndep_files(&dir, std::slice::from_ref(&current))?;
        prune_dyndep_sidecars(&dir, &lease, std::slice::from_ref(&current), policy)?;
        latest_sidecar = Some(current);
        drop(lease);
    }

    let latest = latest_sidecar.context("loop must publish a current sidecar")?;
    let count = sidecar_names(&dir)?
        .iter()
        .filter(|path| has_extension(path, "dd"))
        .count();
    ensure!(
        count <= policy.max_files + 1,
        "the current bundle plus obsolete sidecars must fit the count policy"
    );
    ensure!(
        dir.open(latest.relative_path()).is_ok(),
        "retention must preserve the latest bundle's sidecar"
    );
    Ok(())
}

#[test]
fn repeated_publication_respects_the_obsolete_byte_budget() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temporary_dir(&temp)?;
    let policy = RetentionPolicy::new(8, 12);
    let content = "12345678";
    let mut latest_sidecar = None;

    for index in 0..6 {
        let path = format!(".netsuke/dyndep/bytes-{index}.dd");
        let current = sidecar(&path, content);
        let lease = materialize_dyndep_files(&dir, std::slice::from_ref(&current))?;
        prune_dyndep_sidecars(&dir, &lease, std::slice::from_ref(&current), policy)?;
        latest_sidecar = Some(current);
        drop(lease);
    }

    let latest = latest_sidecar.context("loop must publish a current sidecar")?;
    ensure!(
        retained_sidecar_bytes(&dir)? <= policy.max_bytes + latest.content().len() as u64,
        "the current bundle plus obsolete sidecars must fit the byte policy"
    );
    ensure!(
        dir.open(latest.relative_path()).is_ok(),
        "retention must preserve the latest bundle's sidecar"
    );
    Ok(())
}

#[test]
fn retention_preserves_sidecars_selected_by_overlapping_bundles() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temporary_dir(&temp)?;
    let first = sidecar(".netsuke/dyndep/first.dd", "first");
    let second = sidecar(".netsuke/dyndep/second.dd", "second");
    let bundle = [first, second];
    let lease = materialize_dyndep_files(&dir, &bundle)?;

    prune_dyndep_sidecars(&dir, &lease, &bundle, RetentionPolicy::new(0, 0))?;

    for sidecar in &bundle {
        ensure!(
            dir.open(sidecar.relative_path()).is_ok(),
            "retention must not remove a sidecar selected by an active bundle"
        );
    }
    Ok(())
}

#[test]
fn retention_removes_stale_temporary_files_after_the_lease_is_acquired() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temporary_dir(&temp)?;
    let current = sidecar(".netsuke/dyndep/current.dd", "current");
    let lease = materialize_dyndep_files(&dir, std::slice::from_ref(&current))?;
    let temporary = Utf8Path::new(".netsuke/dyndep/leftover.tmp");
    dir.write(temporary, "interrupted write")?;

    prune_dyndep_sidecars(
        &dir,
        &lease,
        std::slice::from_ref(&current),
        RetentionPolicy::new(0, 0),
    )?;

    ensure!(
        dir.open(temporary).is_err(),
        "retention must remove temporary files outside an active write"
    );
    Ok(())
}

#[test]
fn retention_cleanup_failure_has_localized_context() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temporary_dir(&temp)?;
    let current = sidecar(".netsuke/dyndep/current.dd", "current");
    let lease = materialize_dyndep_files(&dir, std::slice::from_ref(&current))?;
    let failing_path = Utf8Path::new(".netsuke/dyndep/unremovable.dd");
    dir.create_dir_all(failing_path)?;

    let result = prune_dyndep_sidecars(
        &dir,
        &lease,
        std::slice::from_ref(&current),
        RetentionPolicy::new(0, 0),
    );
    let Err(error) = result else {
        anyhow::bail!("retention must report an unremovable candidate");
    };
    let expected = localization::message(keys::RUNNER_IO_DYNDEP_RETENTION)
        .with_arg("path", failing_path.as_str())
        .to_string();
    ensure!(
        format!("{error:#}").contains(&expected),
        "retention failures must retain localized context: {error:#}"
    );
    Ok(())
}
