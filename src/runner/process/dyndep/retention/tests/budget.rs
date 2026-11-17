//! Retention budget and selection-order tests.

use super::*;

/// Write `sidecars` as `x`-filled `.dd` files under the generated-state directory.
///
/// Each entry's size is its content length, which is what retention reads from
/// metadata, so a caller states the budget arithmetic directly instead of
/// spelling out file contents.
fn write_sidecars(dir: &Dir, sidecars: &[(&str, usize)]) -> Result<()> {
    for (name, length) in sidecars {
        dir.write(format!("{DYNDEP_DIR}/{name}"), "x".repeat(*length))?;
    }
    Ok(())
}

/// Return the directory's `.dd` entries in ascending path order.
fn dd_entries(dir: &Dir) -> Result<Vec<Utf8PathBuf>> {
    let mut entries = sidecar_names(dir)?
        .into_iter()
        .filter(|path| has_extension(path, "dd"))
        .collect::<Vec<_>>();
    entries.sort();
    Ok(entries)
}

/// Assert that `dir` retains exactly `expected` across its `.dd` entries.
fn assert_retained(dir: &Dir, expected: &[&str], message: &str) -> Result<()> {
    let retained = dd_entries(dir)?;
    let wanted = expected
        .iter()
        .map(|name| Utf8PathBuf::from(format!("{DYNDEP_DIR}/{name}")))
        .collect::<Vec<_>>();
    ensure!(retained == wanted, "{message}");
    Ok(())
}

/// Prune the workspace through the published bundle's lease with `policy`.
fn prune(published: &PublishedCurrentSidecar, policy: RetentionPolicy) -> Result<RetentionSummary> {
    prune_dyndep_sidecars(
        &published.dir,
        &published.lease,
        std::slice::from_ref(&published.current),
        policy,
    )
}

/// Which obsolete-retention budget a repeated-publication case exercises.
///
/// The two budgets measure different quantities but assert the same shape, so
/// one case table covers both instead of duplicating the publication loop.
#[derive(Clone, Copy, Debug)]
enum ObsoleteBudget {
    /// The number of `.dd` entries, including the current bundle.
    Files,
    /// The bytes of `.dd` entries, including the current bundle.
    Bytes,
}

impl ObsoleteBudget {
    /// Return how much retention used, and the allowance that bounds it.
    ///
    /// Both totals count the current bundle, which retention never removes, so
    /// a case compares an inclusive measurement with an inclusive allowance
    /// rather than mixing it with an obsolete-only budget.
    fn usage(
        self,
        dir: &Dir,
        current: &GeneratedDyndep,
        policy: RetentionPolicy,
    ) -> Result<(u64, u64)> {
        match self {
            Self::Files => {
                let used = u64::try_from(dd_entries(dir)?.len())
                    .expect("a directory holds fewer entries than u64 can count");
                let allowed = u64::try_from(policy.max_files)
                    .expect("a retention budget fits in u64")
                    .saturating_add(1);
                Ok((used, allowed))
            }
            Self::Bytes => {
                let current_bytes = current.content().len() as u64;
                Ok((
                    retained_sidecar_bytes(dir)?,
                    policy.max_bytes.saturating_add(current_bytes),
                ))
            }
        }
    }
}

#[rstest]
#[case::file_count(
    ObsoleteBudget::Files,
    SidecarSeries::new("count", "content", 6),
    RetentionPolicy::new(2, 1_024)
)]
#[case::byte_total(
    ObsoleteBudget::Bytes,
    SidecarSeries::new("bytes", "12345678", 6),
    RetentionPolicy::new(8, 12)
)]
fn repeated_publication_respects_the_obsolete_budget(
    dyndep_workspace: Result<(tempfile::TempDir, Dir)>,
    #[case] budget: ObsoleteBudget,
    #[case] series: SidecarSeries<'_>,
    #[case] policy: RetentionPolicy,
) -> Result<()> {
    let (_workspace, dir) = dyndep_workspace?;
    let latest = publish_repeated_sidecars(&dir, series, policy)?;
    let (used, allowed) = budget.usage(&dir, &latest, policy)?;
    ensure!(
        used <= allowed,
        "repeated publication kept {used} under the {budget:?} budget, above its allowance of {allowed}"
    );
    Ok(())
}

#[rstest]
fn retention_scans_a_large_directory_in_deterministic_path_order(
    published_current_sidecar: Result<PublishedCurrentSidecar>,
) -> Result<()> {
    let published = published_current_sidecar?;
    for index in 0..1_000 {
        published
            .dir
            .write(format!("{DYNDEP_DIR}/stale-{index:04}.dd"), "x")?;
    }

    prune(&published, RetentionPolicy::new(2, 1_024))?;

    assert_retained(
        &published.dir,
        &["current.dd", "stale-0000.dd", "stale-0001.dd"],
        "retention must keep the current sidecar and first two obsolete paths",
    )?;
    ensure!(published.dir.open(".netsuke/dyndep/stale-0002.dd").is_err());
    ensure!(published.dir.open(".netsuke/dyndep/stale-0999.dd").is_err());
    Ok(())
}

#[rstest]
fn retention_reclaims_many_sidecars_larger_than_the_remaining_budget(
    published_current_sidecar: Result<PublishedCurrentSidecar>,
) -> Result<()> {
    let published = published_current_sidecar?;
    for index in 0..1_000 {
        published
            .dir
            .write(format!("{DYNDEP_DIR}/oversized-{index:04}.dd"), "xx")?;
    }

    prune(&published, RetentionPolicy::new(2, 1))?;

    ensure!(
        published
            .dir
            .open(published.current.relative_path())
            .is_ok(),
        "retention must preserve the current bundle's sidecar"
    );
    assert_retained(
        &published.dir,
        &["current.dd"],
        "only the current sidecar must survive an exhausted byte budget",
    )?;
    ensure!(
        published
            .dir
            .open(".netsuke/dyndep/oversized-0000.dd")
            .is_err()
    );
    ensure!(
        published
            .dir
            .open(".netsuke/dyndep/oversized-0999.dd")
            .is_err()
    );
    Ok(())
}

#[rstest]
fn retention_uses_lexical_selection_when_directory_entries_are_nonlexical(
    published_current_sidecar: Result<PublishedCurrentSidecar>,
) -> Result<()> {
    let published = published_current_sidecar?;
    write_sidecars(
        &published.dir,
        &[("a-large.dd", 10), ("b-small.dd", 5), ("c-small.dd", 5)],
    )?;
    let dyndep = published.dir.open_dir(DYNDEP_DIR)?;

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

    assert_retained(
        &published.dir,
        &["a-large.dd", "current.dd"],
        "lexical selection must keep the earliest path and reclaim the later two",
    )?;
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
    let published = published_current_sidecar?;
    write_sidecars(
        &published.dir,
        &[
            ("m.dd", 1),
            ("c.dd", 1),
            ("z.dd", 1),
            ("a.dd", 1),
            ("q.dd", 1),
        ],
    )?;
    let dyndep = published.dir.open_dir(DYNDEP_DIR)?;

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
    let published = published_current_sidecar?;
    let full_budget =
        usize::try_from(MAX_RETAINED_DYNDEP_BYTES).expect("the standard byte budget fits a usize");
    write_sidecars(
        &published.dir,
        &[
            ("zz-full-budget.dd", full_budget),
            ("a-first.dd", 1),
            ("b-second.dd", 1),
            ("c-third.dd", 1),
        ],
    )?;

    prune(&published, RetentionPolicy::standard())?;

    assert_retained(
        &published.dir,
        &["a-first.dd", "b-second.dd", "c-third.dd", "current.dd"],
        "the standard budget must retain the small paths and reclaim the full-budget one",
    )?;
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
    let published = published_current_sidecar?;
    // Sizes follow the CR regression: with max_files 4 and max_bytes 14 only
    // `a.dd` and `f.dd` fit, and `b.dd` alone would exhaust the byte budget.
    write_sidecars(
        &published.dir,
        &[
            ("b.dd", 14),
            ("d.dd", 32),
            ("c.dd", 19),
            ("f.dd", 3),
            ("a.dd", 3),
        ],
    )?;

    prune(&published, RetentionPolicy::new(4, 14))?;

    assert_retained(
        &published.dir,
        &["a.dd", "current.dd", "f.dd"],
        "retention must apply the lexical greedy rule, not the creation order",
    )?;
    Ok(())
}
