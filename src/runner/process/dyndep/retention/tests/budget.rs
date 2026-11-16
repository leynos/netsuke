//! Retention budget and selection-order tests.

use super::*;
#[rstest]
fn repeated_publication_respects_the_obsolete_file_count_budget(
    dyndep_workspace: Result<(tempfile::TempDir, Dir)>,
) -> Result<()> {
    let (_workspace, dir) = dyndep_workspace?;
    let policy = RetentionPolicy::new(2, 1024);
    let latest =
        publish_repeated_sidecars(&dir, SidecarSeries::new("count", "content", 6), policy)?;
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

/// The traversal must present entry names in ascending path order.
///
/// Selection can only be order-independent if the names it sees are already
/// ordered, and a directory lists entries in whatever order it likes. Asserting
/// the ordering directly makes this fail wherever the sort is dropped, rather
/// than only on filesystems that happen to list entries in creation order.
#[rstest]
fn retention_orders_directory_entries_by_path(
    published_current_sidecar: Result<PublishedCurrentSidecar>,
) -> Result<()> {
    let PublishedCurrentSidecar {
        _workspace, dir, ..
    } = published_current_sidecar?;
    for name in ["m.dd", "c.dd", "z.dd", "a.dd", "q.dd"] {
        dir.write(format!(".netsuke/dyndep/{name}"), "x")?;
    }
    let dyndep = dir.open_dir(DYNDEP_DIR)?;

    let names = entry_names(&dyndep)?;
    let mut expected = names.clone();
    expected.sort();

    ensure!(
        !names.is_empty(),
        "the traversal must report the directory's entries"
    );
    ensure!(
        names == expected,
        "the traversal must present entry names in ascending path order"
    );
    Ok(())
}

/// The standard budget follows path order, not the order entries are read.
///
/// A sidecar that fills the whole 1 MiB allowance and is created first leaves no
/// capacity for the small sidecars that follow, and bounded selection could
/// never reconsider a small one it had rejected. Sorting first admits the small
/// paths whose names sort before the large one and reclaims the large one, which
/// is what ADR-0012 means by considering obsolete files in deterministic path
/// order.
#[rstest]
fn retention_follows_path_order_under_the_standard_budget(
    published_current_sidecar: Result<PublishedCurrentSidecar>,
) -> Result<()> {
    let PublishedCurrentSidecar {
        _workspace,
        dir,
        current,
        lease,
    } = published_current_sidecar?;
    let full_budget =
        usize::try_from(MAX_RETAINED_DYNDEP_BYTES).expect("the standard byte budget fits a usize");
    dir.write(".netsuke/dyndep/zz-full-budget.dd", "x".repeat(full_budget))?;
    for name in ["a-first.dd", "b-second.dd", "c-third.dd"] {
        dir.write(format!(".netsuke/dyndep/{name}"), "x")?;
    }

    prune_dyndep_sidecars(
        &dir,
        &lease,
        std::slice::from_ref(&current),
        RetentionPolicy::standard(),
    )?;

    let mut retained = sidecar_names(&dir)?
        .into_iter()
        .filter(|path| has_extension(path, "dd"))
        .filter(|path| path.file_name() != Some("current.dd"))
        .collect::<Vec<_>>();
    retained.sort();
    ensure!(
        retained
            == [
                Utf8PathBuf::from(".netsuke/dyndep/a-first.dd"),
                Utf8PathBuf::from(".netsuke/dyndep/b-second.dd"),
                Utf8PathBuf::from(".netsuke/dyndep/c-third.dd"),
            ],
        "the standard budget must retain the small paths and reclaim the full-budget one"
    );
    Ok(())
}

/// Retained sidecars must not depend on the order the filesystem lists entries.
///
/// The sizes and creation order are the case that makes an order-sensitive walk
/// disagree with the selection rule: `b.dd` created first consumes the whole
/// byte budget, so `f.dd` arrives to no remaining capacity, and `a.dd` created
/// last displaces `b.dd` rather than filling the gap. The pass must still
/// retain exactly what an ascending walk retains, whichever order the
/// filesystem reports.
#[rstest]
fn retention_selects_sidecars_by_path_not_by_creation_order(
    published_current_sidecar: Result<PublishedCurrentSidecar>,
) -> Result<()> {
    let PublishedCurrentSidecar {
        _workspace,
        dir,
        current,
        lease,
    } = published_current_sidecar?;
    // Sizes follow the CR regression: with max_files 4 and max_bytes 14 only
    // `a.dd` and `f.dd` fit, and `b.dd` alone would exhaust the byte budget.
    for (name, len) in [
        ("b.dd", 14),
        ("d.dd", 32),
        ("c.dd", 19),
        ("f.dd", 3),
        ("a.dd", 3),
    ] {
        dir.write(format!(".netsuke/dyndep/{name}"), "x".repeat(len))?;
    }

    prune_dyndep_sidecars(
        &dir,
        &lease,
        std::slice::from_ref(&current),
        RetentionPolicy::new(4, 14),
    )?;

    let mut retained = sidecar_names(&dir)?
        .into_iter()
        .filter(|path| has_extension(path, "dd"))
        .collect::<Vec<_>>();
    retained.sort();
    ensure!(
        retained
            == [
                Utf8PathBuf::from(".netsuke/dyndep/a.dd"),
                Utf8PathBuf::from(".netsuke/dyndep/current.dd"),
                Utf8PathBuf::from(".netsuke/dyndep/f.dd"),
            ],
        "retention must apply the lexical greedy rule, not the creation order"
    );
    Ok(())
}

#[rstest]
fn repeated_publication_respects_the_obsolete_byte_budget(
    dyndep_workspace: Result<(tempfile::TempDir, Dir)>,
) -> Result<()> {
    let (_workspace, dir) = dyndep_workspace?;
    let policy = RetentionPolicy::new(8, 12);
    let latest =
        publish_repeated_sidecars(&dir, SidecarSeries::new("bytes", "12345678", 6), policy)?;
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
