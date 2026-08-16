//! Focused tests for bounded dyndep sidecar retention.

use super::*;
use crate::ninja_gen::GeneratedDyndep;
use crate::runner::process::dyndep_telemetry::{
    RETAINED_BYTES_RECLAIMED, RETAINED_FILES_RECLAIMED, RETENTIONS_TOTAL,
};
use crate::runner::process::materialize_dyndep_files;
use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use cap_std::fs_utf8::Dir;
use rstest::{fixture, rstest};

fn temporary_dir(temp: &tempfile::TempDir) -> Result<Dir> {
    let path = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("temporary directory is not UTF-8: {}", path.display()))?;
    Dir::open_ambient_dir(path, cap_std::ambient_authority()).map_err(Into::into)
}

#[fixture]
fn dyndep_workspace() -> Result<(tempfile::TempDir, Dir)> {
    let workspace = tempfile::tempdir()?;
    let dir = temporary_dir(&workspace)?;
    Ok((workspace, dir))
}

struct PublishedCurrentSidecar {
    _workspace: tempfile::TempDir,
    dir: Dir,
    current: GeneratedDyndep,
    lease: DyndepPublicationLease,
}

#[fixture]
fn published_current_sidecar(
    dyndep_workspace: Result<(tempfile::TempDir, Dir)>,
) -> Result<PublishedCurrentSidecar> {
    let (workspace, dir) = dyndep_workspace?;
    let current = sidecar(".netsuke/dyndep/current.dd", "current");
    let lease = materialize_dyndep_files(&dir, std::slice::from_ref(&current))?;
    Ok(PublishedCurrentSidecar {
        _workspace: workspace,
        dir,
        current,
        lease,
    })
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

fn publish_repeated_sidecars(
    dir: &Dir,
    prefix: &str,
    content: &str,
    policy: RetentionPolicy,
) -> Result<GeneratedDyndep> {
    let mut latest_sidecar = None;

    for index in 0..6 {
        let path = format!(".netsuke/dyndep/{prefix}-{index}.dd");
        let current = sidecar(&path, content);
        let lease = materialize_dyndep_files(dir, std::slice::from_ref(&current))?;
        prune_dyndep_sidecars(dir, &lease, std::slice::from_ref(&current), policy)?;
        latest_sidecar = Some(current);
        drop(lease);
    }

    latest_sidecar.context("loop must publish a current sidecar")
}

#[path = "dyndep_retention_lease_tests.rs"]
mod lease_tests;

#[path = "dyndep_retention_telemetry_tests.rs"]
mod telemetry_tests;

#[rstest]
fn repeated_publication_respects_the_obsolete_file_count_budget(
    dyndep_workspace: Result<(tempfile::TempDir, Dir)>,
) -> Result<()> {
    let (_workspace, dir) = dyndep_workspace?;
    let policy = RetentionPolicy::new(2, 1024);
    let latest = publish_repeated_sidecars(&dir, "count", "content", policy)?;
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

#[rstest]
fn retention_scans_a_large_directory_in_deterministic_path_order(
    published_current_sidecar: Result<PublishedCurrentSidecar>,
) -> Result<()> {
    let PublishedCurrentSidecar {
        _workspace,
        dir,
        current,
        lease,
    } = published_current_sidecar?;
    for index in 0..1_000 {
        let path = format!(".netsuke/dyndep/stale-{index:04}.dd");
        dir.write(path, "x")?;
    }
    prune_dyndep_sidecars(
        &dir,
        &lease,
        std::slice::from_ref(&current),
        RetentionPolicy::new(2, 1_024),
    )?;
    let mut retained = sidecar_names(&dir)?
        .into_iter()
        .filter(|path| has_extension(path, "dd"))
        .collect::<Vec<_>>();
    retained.sort();
    ensure!(
        retained
            == [
                Utf8PathBuf::from(".netsuke/dyndep/current.dd"),
                Utf8PathBuf::from(".netsuke/dyndep/stale-0000.dd"),
                Utf8PathBuf::from(".netsuke/dyndep/stale-0001.dd"),
            ],
        "retention must keep the current sidecar and first two obsolete paths"
    );
    ensure!(dir.open(".netsuke/dyndep/stale-0002.dd").is_err());
    ensure!(dir.open(".netsuke/dyndep/stale-0999.dd").is_err());
    Ok(())
}

#[rstest]
fn retention_reclaims_many_sidecars_larger_than_the_remaining_budget(
    published_current_sidecar: Result<PublishedCurrentSidecar>,
) -> Result<()> {
    let PublishedCurrentSidecar {
        _workspace,
        dir,
        current,
        lease,
    } = published_current_sidecar?;
    for index in 0..1_000 {
        dir.write(format!(".netsuke/dyndep/oversized-{index:04}.dd"), "xx")?;
    }

    prune_dyndep_sidecars(
        &dir,
        &lease,
        std::slice::from_ref(&current),
        RetentionPolicy::new(2, 1),
    )?;

    let sidecars = sidecar_names(&dir)?
        .into_iter()
        .filter(|path| has_extension(path, "dd"))
        .count();
    ensure!(dir.open(current.relative_path()).is_ok());
    ensure!(sidecars == 1, "only the current sidecar must remain");
    ensure!(dir.open(".netsuke/dyndep/oversized-0000.dd").is_err());
    ensure!(dir.open(".netsuke/dyndep/oversized-0999.dd").is_err());
    Ok(())
}

#[rstest]
fn retention_uses_lexical_selection_when_directory_entries_are_nonlexical(
    published_current_sidecar: Result<PublishedCurrentSidecar>,
) -> Result<()> {
    let PublishedCurrentSidecar {
        _workspace,
        dir,
        current,
        ..
    } = published_current_sidecar?;
    let earliest = Utf8PathBuf::from(".netsuke/dyndep/a-large.dd");
    let later_first = Utf8PathBuf::from(".netsuke/dyndep/b-small.dd");
    let later_second = Utf8PathBuf::from(".netsuke/dyndep/c-small.dd");
    dir.write(&earliest, "0123456789")?;
    dir.write(&later_first, "12345")?;
    dir.write(&later_second, "67890")?;

    let mut selection = RetentionSelection::new(RetentionPolicy::new(2, 10));
    let mut summary = RetentionSummary::default();
    retain_or_remove_sidecar(&dir, later_first, &mut selection, &mut summary)?;
    retain_or_remove_sidecar(&dir, later_second, &mut selection, &mut summary)?;
    retain_or_remove_sidecar(&dir, earliest.clone(), &mut selection, &mut summary)?;

    ensure!(dir.open(current.relative_path()).is_ok());
    ensure!(dir.open(&earliest).is_ok());
    ensure!(dir.open(".netsuke/dyndep/b-small.dd").is_err());
    ensure!(dir.open(".netsuke/dyndep/c-small.dd").is_err());
    Ok(())
}

#[rstest]
fn repeated_publication_respects_the_obsolete_byte_budget(
    dyndep_workspace: Result<(tempfile::TempDir, Dir)>,
) -> Result<()> {
    let (_workspace, dir) = dyndep_workspace?;
    let policy = RetentionPolicy::new(8, 12);
    let latest = publish_repeated_sidecars(&dir, "bytes", "12345678", policy)?;
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

#[rstest]
fn retention_removes_stale_temporary_files_after_the_lease_is_acquired(
    published_current_sidecar: Result<PublishedCurrentSidecar>,
) -> Result<()> {
    let PublishedCurrentSidecar {
        _workspace,
        dir,
        current,
        lease,
    } = published_current_sidecar?;
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

#[rstest]
fn retention_cleanup_failure_has_localized_context(
    published_current_sidecar: Result<PublishedCurrentSidecar>,
) -> Result<()> {
    let PublishedCurrentSidecar {
        _workspace,
        dir,
        current,
        lease,
    } = published_current_sidecar?;
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
