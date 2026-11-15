//! Retention entry handling and alias-refusal tests.

use super::*;
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
fn empty_lease_leaves_obsolete_sidecars_untouched() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temporary_dir(&temp)?;
    let stale = Utf8Path::new(".netsuke/dyndep/stale.dd");
    dir.create_dir_all(DYNDEP_DIR)?;
    dir.write(stale, "stale")?;

    let summary = DyndepPublicationLease::empty().prune(&[])?;

    ensure!(
        dir.open(stale).is_ok(),
        "an empty lease must not prune sidecars"
    );
    ensure!(
        summary.reclaimed_files == 0,
        "an empty lease must reclaim no files"
    );
    ensure!(
        summary.reclaimed_bytes == 0,
        "an empty lease must reclaim no bytes"
    );
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
        .with_arg("path", entry_display_path("unremovable.dd").as_str())
        .to_string();
    ensure!(
        format!("{error:#}").contains(&expected),
        "retention failures must retain localized context: {error:#}"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn prune_rejects_a_dyndep_directory_symlink(
    dyndep_workspace: Result<(tempfile::TempDir, Dir)>,
) -> Result<()> {
    let (workspace, dir) = dyndep_workspace?;
    let victim = workspace.path().join("victim");
    test_support::fs::create_dir(&victim)?;
    test_support::fs::write(victim.join("keep.dd"), "keep")?;
    test_support::fs::write(victim.join("stale.tmp"), "stale")?;
    for index in 0..40 {
        test_support::fs::write(victim.join(format!("stale-{index:02}.dd")), "stale")?;
    }
    test_support::fs::create_dir(workspace.path().join(".netsuke"))?;
    test_support::fs::symlink("../victim", workspace.path().join(".netsuke/dyndep"))?;

    let result = prune_dyndep_cache(&dir, &[]);

    let Err(error) = result else {
        anyhow::bail!("retention must reject a dyndep directory symlink");
    };
    let expected = localization::message(keys::RUNNER_IO_DYNDEP_ALIAS)
        .with_arg("path", DYNDEP_DIR)
        .to_string();
    ensure!(
        format!("{error:#}").contains(&expected),
        "the refusal must name the aliased component: {error:#}"
    );
    for relative in ["keep.dd", "stale.tmp", "stale-00.dd", "stale-39.dd"] {
        ensure!(
            test_support::fs::exists(victim.join(relative)),
            "retention must not delete {relative} through the alias"
        );
    }
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn prune_rejects_a_netsuke_directory_symlink(
    dyndep_workspace: Result<(tempfile::TempDir, Dir)>,
) -> Result<()> {
    let (workspace, dir) = dyndep_workspace?;
    let victim = workspace.path().join("victim");
    test_support::fs::create_dir_all(victim.join("dyndep"))?;
    test_support::fs::write(victim.join("dyndep/old.dd"), "old")?;
    test_support::fs::write(victim.join("dyndep/old.tmp"), "old")?;
    test_support::fs::symlink("victim", workspace.path().join(".netsuke"))?;

    let result = prune_dyndep_cache(&dir, &[]);

    let Err(error) = result else {
        anyhow::bail!("retention must reject a .netsuke directory symlink");
    };
    let expected = localization::message(keys::RUNNER_IO_DYNDEP_ALIAS)
        .with_arg("path", ".netsuke")
        .to_string();
    ensure!(
        format!("{error:#}").contains(&expected),
        "the refusal must name the aliased component: {error:#}"
    );
    ensure!(
        test_support::fs::exists(victim.join("dyndep/old.dd")),
        "retention must not delete files through the .netsuke alias"
    );
    ensure!(
        test_support::fs::exists(victim.join("dyndep/old.tmp")),
        "retention must not delete temporary files through the .netsuke alias"
    );
    Ok(())
}

/// Replacing the verified directory after the lease is acquired must not
/// redirect retention at the new occupant.
///
/// The lease holds the handle opened by `open_component`, so retention
/// addresses bare entry names in the directory that handle names. Swapping the
/// path afterwards cannot make the replacement disposable: pruning removes
/// obsolete sidecars from the directory that was verified and never resolves
/// the alias now sitting at the original path. This is the second half of the
/// double check: an entry swapped between the pre-open inspection and the open
/// is refused, and an entry swapped after the open is simply not the inode the
/// handle names.
#[cfg(unix)]
#[rstest]
fn retention_prunes_the_verified_directory_after_a_path_replacement(
    published_current_sidecar: Result<PublishedCurrentSidecar>,
) -> Result<()> {
    let PublishedCurrentSidecar {
        _workspace: workspace,
        dir,
        current,
        lease,
    } = published_current_sidecar?;
    for index in 0..40 {
        dir.write(
            format!(".netsuke/dyndep/obsolete-{index:02}.dd"),
            "obsolete",
        )?;
    }

    let victim = workspace.path().join("victim");
    test_support::fs::create_dir(&victim)?;
    test_support::fs::write(victim.join("keep.dd"), "keep")?;
    test_support::fs::write(victim.join("stale.tmp"), "stale")?;

    dir.rename(".netsuke/dyndep", &dir, ".netsuke/dyndep-original")?;
    test_support::fs::symlink("../victim", workspace.path().join(".netsuke/dyndep"))?;

    let summary = lease.prune(std::slice::from_ref(&current))?;

    ensure!(
        summary.reclaimed_files == 8,
        "retention must reclaim the 8 obsolete sidecars beyond the 32-file budget, got {}",
        summary.reclaimed_files
    );
    let verified = workspace.path().join(".netsuke/dyndep-original");
    ensure!(
        !test_support::fs::exists(verified.join("obsolete-39.dd")),
        "retention must prune the directory the handle verified, not the path"
    );
    ensure!(
        test_support::fs::exists(verified.join("obsolete-00.dd")),
        "the directory the handle verified must retain its budget of sidecars"
    );
    ensure!(
        test_support::fs::exists(verified.join("current.dd")),
        "the current sidecar must survive in the verified directory"
    );
    ensure!(
        test_support::fs::exists(victim.join("keep.dd")),
        "replacing the path after verification must not make the new target disposable"
    );
    ensure!(
        test_support::fs::exists(victim.join("stale.tmp")),
        "temporary-file pruning must not reach the replacement target"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn retention_skips_symlink_entries_and_preserves_their_targets() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temporary_dir(&temp)?;
    let outside = tempfile::tempdir()?;
    let target = outside.path().join("precious.dd");
    test_support::fs::write(&target, "precious")?;
    let current = sidecar(".netsuke/dyndep/current.dd", "current");
    let lease = materialize_dyndep_files(&dir, std::slice::from_ref(&current))?;
    let alias = temp.path().join(".netsuke/dyndep/alias.dd");
    test_support::fs::symlink(&target, &alias)?;

    prune_dyndep_sidecars(
        &dir,
        &lease,
        std::slice::from_ref(&current),
        RetentionPolicy::new(0, 0),
    )?;

    ensure!(
        test_support::fs::read_to_string(&target)? == "precious",
        "a symlink entry must never make its target disposable"
    );
    ensure!(
        test_support::fs::is_symlink(&alias),
        "retention must skip a symlink entry rather than delete or follow it"
    );
    Ok(())
}

#[cfg(windows)]
#[expect(
    clippy::print_stderr,
    reason = "test harness: a host without cmd.exe must report the skip in the captured test output instead of passing silently"
)]
#[rstest]
fn retention_rejects_a_dyndep_directory_junction(
    dyndep_workspace: Result<(tempfile::TempDir, Dir)>,
) -> Result<()> {
    let (workspace, dir) = dyndep_workspace?;
    let victim = workspace.path().join("victim");
    test_support::fs::create_dir(&victim)?;
    test_support::fs::write(victim.join("keep.dd"), "keep")?;
    test_support::fs::write(victim.join("stale.tmp"), "stale")?;
    test_support::fs::create_dir(workspace.path().join(".netsuke"))?;
    let created = test_support::fs::junction(&victim, workspace.path().join(".netsuke/dyndep"))?;
    if !created {
        eprintln!("skipping junction retention test: cmd.exe is unavailable");
        return Ok(());
    }

    let result = prune_dyndep_cache(&dir, &[]);

    ensure!(
        result.is_err(),
        "retention must reject a dyndep directory junction"
    );
    ensure!(
        test_support::fs::exists(victim.join("keep.dd")),
        "retention must not delete files through the junction"
    );
    ensure!(
        test_support::fs::exists(victim.join("stale.tmp")),
        "retention must not delete temporary files through the junction"
    );
    Ok(())
}

#[cfg(windows)]
#[test]
fn windows_retention_removes_stale_sidecars_by_native_path_identity() -> Result<()> {
    // Retention now compares bare directory-entry names, so a sidecar
    // addressed through the native Windows separator spelling must still be
    // recognised and pruned. The assertion resolves both paths through the
    // capability directory, so NTFS treats them as the files on disk.
    let temp = tempfile::tempdir()?;
    let dir = temporary_dir(&temp)?;
    let current = sidecar(".netsuke/dyndep/current.dd", "current");
    let lease = materialize_dyndep_files(&dir, std::slice::from_ref(&current))?;

    // A stale sidecar is materialised by the same writer, so spell its path
    // the way the NTFS directory entry reports it: backslash separators.
    let stale_native = ".netsuke\\dyndep\\stale-candidate.dd";
    dir.write(stale_native, "stale")?;

    prune_dyndep_sidecars(
        &dir,
        &lease,
        std::slice::from_ref(&current),
        RetentionPolicy::new(0, 0),
    )?;

    ensure!(
        dir.open(current.relative_path()).is_ok(),
        "retention must preserve the current bundle's sidecar under native path identity"
    );
    let stale_open_error = dir
        .open(stale_native)
        .expect_err("retention must remove a stale sidecar reported with native separators");
    ensure!(
        stale_open_error.kind() == std::io::ErrorKind::NotFound,
        "stale sidecar must be absent rather than inaccessible: {stale_open_error}"
    );
    Ok(())
}
