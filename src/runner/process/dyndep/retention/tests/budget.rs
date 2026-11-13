//! Retention budget and selection-order tests.

use super::*;
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
    let dyndep = dir.open_dir(DYNDEP_DIR)?;

    let current_names = HashSet::new();
    let mut summary = RetentionSummary::default();
    let mut pass = RetentionPass {
        current_names: &current_names,
        retained: RetentionSelection::new(RetentionPolicy::new(2, 10)),
        summary: &mut summary,
    };
    // The sizes mirror the file contents written above.
    retain_or_remove_sidecar(&dyndep, "b-small.dd", 5, &mut pass)?;
    retain_or_remove_sidecar(&dyndep, "c-small.dd", 5, &mut pass)?;
    retain_or_remove_sidecar(&dyndep, "a-large.dd", 10, &mut pass)?;

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
