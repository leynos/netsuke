//! Symlink and file-type policy tests for the file-reading filters.
//!
//! The default policy opens the final path component without following
//! symlinks and rejects anything that is not a regular file; the
//! `follow_symlinks` opt-in resolves a link but must not reintroduce a
//! blocking open. Both clauses are asserted for every entry point through the
//! shared filter table.
use anyhow::{Context, Result, ensure};
use minijinja::ErrorKind;
use rstest::rstest;
use test_support::fluent::normalize_fluent_isolates;

#[cfg(unix)]
use super::create_fifo;
use super::fallible;
use super::{
    CONTENTS, DIGEST, FilterCase, HASH, LINECOUNT, ReadTarget, rejection, render_case,
    require_real_symlink,
};

#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn reading_filters_reject_symlinks_by_default(#[case] case: FilterCase) -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let Some(link) = fallible::file_symlink_fixture(&root)? else {
        return Ok(()); // This host cannot create symlinks; nothing to police.
    };
    require_real_symlink(&root, &link)?;
    let err = rejection(
        case,
        render_case(
            case,
            "symlink",
            ReadTarget::new(&root, &link, 1024),
            &case.template(),
        )?,
    )?;
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "{}: should report InvalidOperation for a symlink but was {:?}",
        case.name,
        err.kind()
    );
    let message = normalize_fluent_isolates(&err.to_string());
    // O_NOFOLLOW surfaces either as our not-regular-file diagnostic or,
    // through the platform, as an ELOOP-style open failure; both reject
    // the link without reading it.
    ensure!(
        message.contains("not a regular file") || message.contains("symbolic links"),
        "{}: error should explain the rejection: {message}",
        case.name
    );
    Ok(())
}

#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn follow_symlinks_opt_in_reads_the_link_target(#[case] case: FilterCase) -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let Some(link) = fallible::file_symlink_fixture(&root)? else {
        return Ok(());
    };
    require_real_symlink(&root, &link)?;
    let rendered = render_case(
        case,
        "follow",
        ReadTarget::new(&root, &link, 1024),
        &case.template_with("follow_symlinks=true"),
    )?
    .with_context(|| format!("{}: render with follow_symlinks", case.name))?;
    ensure!(
        rendered == case.followed,
        "{}: expected the opt-in to follow the link and render {} but rendered {rendered}",
        case.name,
        case.followed
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn reading_filters_reject_a_fifo(#[case] case: FilterCase) -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let pipe = create_fifo(&root, "pipe")?;
    let err = rejection(
        case,
        render_case(
            case,
            "fifo",
            ReadTarget::new(&root, &pipe, 1024),
            &case.template(),
        )?,
    )?;
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "{}: should report InvalidOperation for a FIFO but was {:?}",
        case.name,
        err.kind()
    );
    Ok(())
}

/// The symlink opt-in must not reintroduce a blocking open.
///
/// `O_NOFOLLOW` is what rejects a FIFO under the default policy, so the
/// follow path is the one that has to carry `O_NONBLOCK` on its own: opening
/// this FIFO for reading with neither a writer nor `O_NONBLOCK` blocks until
/// a writer appears, and the test would only end when `nextest` terminates
/// it. A rejection through the regular-file check proves the open returned.
#[cfg(unix)]
#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn reading_filters_reject_a_fifo_when_following_symlinks(#[case] case: FilterCase) -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let pipe = create_fifo(&root, "pipe")?;
    let err = rejection(
        case,
        render_case(
            case,
            "fifo_follow",
            ReadTarget::new(&root, &pipe, 1024),
            &case.template_with("follow_symlinks=true"),
        )?,
    )?;
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "{}: should report InvalidOperation for a FIFO but was {:?}",
        case.name,
        err.kind()
    );
    let message = normalize_fluent_isolates(&err.to_string());
    ensure!(
        message.contains("not a regular file"),
        "{}: a non-blocking open should reach the regular-file check: {message}",
        case.name
    );
    Ok(())
}
