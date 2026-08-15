//! Unit tests for atomic dyndep sidecar materialization.

use super::*;
use crate::ninja_gen::GeneratedDyndep;
use crate::runner::process::dyndep_retention::{RetentionPolicy, prune_dyndep_sidecars};
use anyhow::{Result, ensure};
use camino::Utf8PathBuf;
use metrics_util::MetricKind;
use metrics_util::debugging::{DebugValue, DebuggingRecorder};

type Snapshot = Vec<(
    metrics_util::CompositeKey,
    Option<metrics::Unit>,
    Option<metrics::SharedString>,
    DebugValue,
)>;

fn sidecar(name: &str, content: &str) -> GeneratedDyndep {
    GeneratedDyndep::fixture(Utf8PathBuf::from(name), content.to_owned())
}

fn temp_dir(temp: &tempfile::TempDir) -> Result<Dir> {
    let path = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow!("temporary directory is not UTF-8: {}", path.display()))?;
    Dir::open_ambient_dir(path, cap_std::ambient_authority()).map_err(Into::into)
}

fn recorded<T>(invoke: impl FnOnce() -> T) -> (T, Snapshot) {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let value = metrics::with_local_recorder(&recorder, invoke);
    (value, snapshotter.snapshot().into_vec())
}

fn counter_value(snapshot: &Snapshot, outcome: &str) -> Option<u64> {
    snapshot
        .iter()
        .find_map(|(key, _unit, _description, value)| {
            if key.kind() != MetricKind::Counter
                || key.key().name() != telemetry::MATERIALIZATIONS_TOTAL
            {
                return None;
            }
            let has_outcome = key
                .key()
                .labels()
                .any(|label| label.key() == "outcome" && label.value() == outcome);
            match value {
                DebugValue::Counter(count) if has_outcome => Some(*count),
                _ => None,
            }
        })
}

fn duration_sample_count(snapshot: &Snapshot) -> usize {
    snapshot
        .iter()
        .find_map(|(key, _unit, _description, value)| {
            if key.kind() != MetricKind::Histogram
                || key.key().name() != telemetry::MATERIALIZATION_DURATION
            {
                return None;
            }
            match value {
                DebugValue::Histogram(samples) => Some(samples.len()),
                _ => None,
            }
        })
        .unwrap_or_default()
}

fn assert_materialization_metrics(snapshot: &Snapshot, outcome: &str) -> Result<()> {
    ensure!(
        counter_value(snapshot, outcome) == Some(1),
        "materialization must record one {outcome} outcome"
    );
    ensure!(
        duration_sample_count(snapshot) == 1,
        "materialization must record one duration sample"
    );
    Ok(())
}

#[test]
fn materializes_nested_sidecar_and_reuses_it() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temp_dir(&temp)?;
    let dyndep = sidecar(".netsuke/dyndep/abc.dd", "ninja_dyndep_version = 1\n");

    materialize_dyndep_files(&dir, &[dyndep])?;
    ensure_matching(&dir, ".netsuke/dyndep/abc.dd", "ninja_dyndep_version = 1\n")?;

    // Second run reuses the existing sidecar without error.
    materialize_dyndep_files(
        &dir,
        &[sidecar(
            ".netsuke/dyndep/abc.dd",
            "ninja_dyndep_version = 1\n",
        )],
    )?;
    Ok(())
}

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

#[test]
fn empty_sidecar_list_does_not_create_dyndep_directory() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temp_dir(&temp)?;

    materialize_dyndep_files(&dir, &[])?;

    ensure!(
        dir.open(DYNDEP_DIR).is_err(),
        "empty sidecar list must not create {DYNDEP_DIR}"
    );
    Ok(())
}

#[test]
fn corrupt_existing_sidecar_is_reported() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temp_dir(&temp)?;
    dir.create_dir_all(DYNDEP_DIR)?;
    dir.write(".netsuke/dyndep/bad.dd", "corrupt")?;

    let result = materialize_dyndep_files(&dir, &[sidecar(".netsuke/dyndep/bad.dd", "expected")]);
    ensure!(result.is_err(), "corrupt sidecar must be reported");
    Ok(())
}

#[test]
fn oversized_existing_sidecar_is_rejected() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temp_dir(&temp)?;
    let rel = ".netsuke/dyndep/oversized.dd";
    dir.create_dir_all(DYNDEP_DIR)?;
    let oversized_size = usize::try_from(MAX_VERIFIED_DYNDEP_SIZE + 1)?;
    dir.write(rel, vec![b'x'; oversized_size])?;

    let Err(error) = materialize_dyndep_files(&dir, &[sidecar(rel, "expected")]) else {
        anyhow::bail!("an oversized existing sidecar must be rejected");
    };
    let expected = localization::message(keys::RUNNER_IO_DYNDEP_TOO_LARGE)
        .with_arg("path", rel)
        .with_arg("limit", MAX_VERIFIED_DYNDEP_SIZE)
        .to_string();
    ensure!(
        format!("{error:#}").contains(&expected),
        "expected localized oversized-sidecar error, got: {error:#}"
    );
    Ok(())
}

#[test]
fn sidecar_growth_during_verification_is_a_mismatch() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temp_dir(&temp)?;
    let rel = Utf8Path::new(".netsuke/dyndep/growing.dd");
    let expected = "expected";
    dir.create_dir_all(DYNDEP_DIR)?;
    dir.write(rel, expected)?;
    let mut file = open_existing_sidecar(&dir, rel)?.context("sidecar must exist")?;
    let size = verified_sidecar_size(&file, rel)?;
    let mut append_options = OpenOptions::new();
    append_options.append(true);
    let mut writer = dir.open_with(rel, &append_options)?;
    writer.write_all(b" growth")?;
    writer.flush()?;

    let (content, grew_while_reading) = read_sidecar_content(&mut file, size, rel)?;

    ensure!(
        grew_while_reading,
        "growth probe must observe appended data"
    );
    ensure!(
        content_outcome(&content, expected, grew_while_reading) == ReadOutcome::Mismatch,
        "a sidecar that grows during verification must not be reused"
    );
    Ok(())
}

#[test]
fn no_temp_files_left_behind() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temp_dir(&temp)?;
    materialize_dyndep_files(&dir, &[sidecar(".netsuke/dyndep/x.dd", "content")])?;
    ensure_no_temp_files(&dir)?;
    Ok(())
}

#[test]
fn failed_atomic_write_removes_temp_file() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temp_dir(&temp)?;
    let rel = Utf8Path::new(".netsuke/dyndep/destination.dd");
    dir.create_dir_all(rel)?;

    let result = write_atomic(&dir, rel, "content");

    ensure!(result.is_err(), "rename over a directory must fail");
    ensure_no_temp_files(&dir)?;
    Ok(())
}

#[test]
fn stale_temp_file_does_not_block_materialization() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temp_dir(&temp)?;
    let rel = Utf8Path::new(".netsuke/dyndep/stale.dd");
    let content = "ninja_dyndep_version = 1\n";
    dir.create_dir_all(DYNDEP_DIR)?;
    let stale = TempNameSource::new("stale".to_owned()).next_name(rel);
    dir.write(stale, "stale temporary content")?;

    materialize_dyndep_files(&dir, &[sidecar(rel.as_str(), content)])?;

    ensure_matching(&dir, rel.as_str(), content)
}

#[test]
fn separate_temp_names_for_same_sidecar_differ() {
    let rel = Utf8Path::new(".netsuke/dyndep/names.dd");
    let mut names = TempNameSource::new("fixture".to_owned());
    let first = names.next_name(rel);
    let second = names.next_name(rel);

    assert_ne!(first, second, "temporary names must differ per attempt");
    assert_eq!(first.parent(), rel.parent());
    assert_eq!(second.parent(), rel.parent());
}

#[test]
fn temporary_name_collision_retries_are_bounded() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temp_dir(&temp)?;
    let rel = Utf8Path::new(".netsuke/dyndep/collisions.dd");
    dir.create_dir_all(DYNDEP_DIR)?;
    let mut occupied_names = TempNameSource::new("collisions".to_owned());
    for _ in 0..MAX_TEMP_FILE_ATTEMPTS {
        dir.write(occupied_names.next_name(rel), "occupied")?;
    }
    let mut attempted_names = TempNameSource::new("collisions".to_owned());

    let error = create_unique_temp_file_with_source(&dir, rel, &mut attempted_names)
        .err()
        .context("all occupied temporary names must exhaust the retry bound")?;
    let expected = localization::message(keys::RUNNER_IO_DYNDEP_TEMP_COLLISIONS)
        .with_arg("path", rel.as_str())
        .to_string();
    ensure!(
        format!("{error:#}").contains(&expected),
        "expected localized collision error, got: {error:#}"
    );
    Ok(())
}

#[test]
fn matching_final_sidecar_succeeds_with_another_temp_file() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temp_dir(&temp)?;
    let rel = Utf8Path::new(".netsuke/dyndep/matching.dd");
    let content = "ninja_dyndep_version = 1\n";
    dir.create_dir_all(DYNDEP_DIR)?;
    dir.write(rel, content)?;
    let other_temp = TempNameSource::new("concurrent".to_owned()).next_name(rel);
    dir.write(other_temp, "concurrent temporary content")?;

    write_atomic(&dir, rel, content)?;

    ensure_matching(&dir, rel.as_str(), content)
}

#[test]
fn retention_prunes_historical_sidecars_but_keeps_the_current_bundle() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temp_dir(&temp)?;
    dir.create_dir_all(DYNDEP_DIR)?;
    dir.write(".netsuke/dyndep/stale-a.dd", "stale-a")?;
    dir.write(".netsuke/dyndep/stale-b.dd", "stale-b")?;
    let current = sidecar(".netsuke/dyndep/current.dd", "current");

    let lease = materialize_dyndep_files(&dir, std::slice::from_ref(&current))?;
    prune_dyndep_sidecars(
        &dir,
        &lease,
        std::slice::from_ref(&current),
        RetentionPolicy::new(1, 8),
    )?;

    ensure_matching(&dir, current.relative_path().as_str(), current.content())?;
    ensure!(
        dir.open(".netsuke/dyndep/stale-a.dd").is_err()
            || dir.open(".netsuke/dyndep/stale-b.dd").is_err(),
        "retention must evict historical sidecars above its file-count budget"
    );
    Ok(())
}

fn ensure_matching(dir: &Dir, path: &str, expected: &str) -> Result<()> {
    anyhow::ensure!(
        dir.read_to_string(path)? == expected,
        "sidecar content does not match"
    );
    Ok(())
}

fn ensure_no_temp_files(dir: &Dir) -> Result<()> {
    let names = dir
        .read_dir(DYNDEP_DIR)?
        .map(|entry| entry.and_then(|item| item.file_name()))
        .collect::<std::io::Result<Vec<_>>>()?;
    ensure!(
        names.iter().all(|name| {
            Utf8Path::new(name)
                .extension()
                .is_none_or(|extension| !extension.eq_ignore_ascii_case("tmp"))
        }),
        "temporary files left behind: {names:?}"
    );
    Ok(())
}
