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

/// Report that this host cannot provide the symlink fixture, then end the case.
///
/// `Ok(None)` from the fixture means the platform cannot create a link at all —
/// on Windows, for want of `SeCreateSymbolicLinkPrivilege` and Developer Mode —
/// so the case has no symlink policy to assert here. It says so rather than
/// returning a silent pass: a suite that reports green while quietly skipping
/// its subject would hide the same regression on a host that can create links.
#[expect(
    clippy::print_stderr,
    reason = "test harness: an unavailable fixture must be visible in the captured test output instead of passing silently"
)]
fn skip_without_symlink_support(case: FilterCase) {
    eprintln!(
        "skipped: {} — this host cannot create a symlink fixture",
        case.name
    );
}

#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn reading_filters_reject_symlinks_by_default(#[case] case: FilterCase) -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let Some(link) = fallible::file_symlink_fixture(&root)? else {
        skip_without_symlink_support(case);
        return Ok(());
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
        skip_without_symlink_support(case);
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
/// `O_NONBLOCK` is what keeps this open from wedging the render worker under
/// either policy: opening a FIFO for reading with neither a writer nor
/// `O_NONBLOCK` blocks until a writer appears, and the test would only end when
/// `nextest` terminates it. `O_NOFOLLOW` is not the guard here — it refuses a
/// symlink final component, and a FIFO is not one. The FIFO therefore reaches
/// the read-only open, returns because of `O_NONBLOCK`, and is rejected by the
/// regular-file check on the opened handle, which is what this case asserts.
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
