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

use super::fallible;
#[cfg(windows)]
use super::require_real_junction;
use super::{
    CONTENTS, DIGEST, FilterCase, HASH, LINECOUNT, Policy, ReadTarget, rejection, render_case,
    require_real_symlink,
};
#[cfg(unix)]
use super::{create_fifo, create_symlink};

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

/// Report that this host cannot provide the junction fixture, then end the case.
///
/// A junction needs no privilege, so `Ok(None)` here means only that `cmd.exe`
/// is absent. The case says so rather than passing silently: a suite that
/// reports green while quietly skipping its subject would hide the same
/// regression on a host that can create the fixture.
#[cfg(windows)]
#[expect(
    clippy::print_stderr,
    reason = "test harness: an unavailable fixture must be visible in the captured test output instead of passing silently"
)]
fn skip_without_junction_support(case: FilterCase) {
    eprintln!(
        "skipped: {} — this host cannot create a junction fixture",
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

/// The Windows default policy must refuse a junction through the same open
/// that yields the handle.
///
/// A junction is a directory-shaped reparse point: reading it back as text
/// renders the target's file names, so a policy that let the open traverse it
/// would silently expose a directory the caller never asked for. This case is
/// the Windows half of the same-handle guarantee, where the decision comes
/// from the opened handle's reparse-point attribute rather than from a
/// pre-open `symlink_metadata` lookup that a concurrent replace could defeat.
/// The explicit spelling of the default policy must refuse a final symlink.
///
/// Native Linux resolves this through `openat2`, where the raw `O_NOFOLLOW`
/// flag already refuses the link. The manual resolver consults cap-std's
/// logical policy instead, so a caller that omits `FollowSymlinks::No` reads
/// the target on macOS and on every fallback path. This case pins the
/// documented `follow_symlinks=false` spelling at the same boundary as the
/// omitted default; `resolver_fallback_tests` forces the manual resolver and
/// provides the pre-fix failure evidence.
#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn reading_filters_reject_symlinks_when_follow_is_false(#[case] case: FilterCase) -> Result<()> {
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
            "symlink_false",
            ReadTarget::new(&root, &link, 1024),
            &Policy::FollowFalse.template(case),
        )?,
    )?;
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "{}: should report InvalidOperation for a symlink but was {:?}",
        case.name,
        err.kind()
    );
    let message = normalize_fluent_isolates(&err.to_string());
    ensure!(
        message.contains("not a regular file") || message.contains("symbolic links"),
        "{}: error should explain the rejection: {message}",
        case.name
    );
    Ok(())
}

/// A regular file must read normally under every spelling of the policy.
///
/// The symlink cases prove a rejection; this case proves the rejection is
/// scoped to the link and does not refuse an ordinary file, under the default,
/// the explicit `false`, or the `true` opt-in.
#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn reading_filters_read_regular_files_under_every_policy(
    #[case] case: FilterCase,
    #[values(Policy::Omitted, Policy::FollowFalse, Policy::FollowTrue)] policy: Policy,
) -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let file = root.join("file");
    let rendered = render_case(
        case,
        &format!("regular_{}", policy.suffix()),
        ReadTarget::new(&root, &file, 1024),
        &policy.template(case),
    )?
    .with_context(|| format!("{}: render a regular file under {:?}", case.name, policy))?;
    ensure!(
        rendered == case.followed,
        "{}: expected {} for a regular file under {:?} but rendered {rendered}",
        case.name,
        case.followed,
        policy
    );
    Ok(())
}

#[cfg(windows)]
#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn reading_filters_reject_a_junction(#[case] case: FilterCase) -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let Some(junction) = fallible::junction_fixture(&root)? else {
        skip_without_junction_support(case);
        return Ok(());
    };
    require_real_junction(&root, &junction)?;
    let err = rejection(
        case,
        render_case(
            case,
            "junction",
            ReadTarget::new(&root, &junction, 1024),
            &case.template(),
        )?,
    )?;
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "{}: should report InvalidOperation for a junction but was {:?}",
        case.name,
        err.kind()
    );
    let message = normalize_fluent_isolates(&err.to_string());
    // The refusal happens on the opened handle, so it surfaces as the
    // regular-file diagnostic; a traversal would have rendered the target
    // directory's entries instead of erroring at all.
    ensure!(
        message.contains("not a regular file"),
        "{}: error should explain the rejection: {message}",
        case.name
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

/// The explicit `false` spelling must refuse a direct FIFO without blocking.
///
/// `O_NOFOLLOW` does not apply to a FIFO, so this case proves the
/// non-blocking open and the regular-file check still hold when the policy is
/// stated explicitly rather than omitted.
#[cfg(unix)]
#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn reading_filters_reject_a_fifo_when_follow_is_false(#[case] case: FilterCase) -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let pipe = create_fifo(&root, "pipe")?;
    let err = rejection(
        case,
        render_case(
            case,
            "fifo_false",
            ReadTarget::new(&root, &pipe, 1024),
            &Policy::FollowFalse.template(case),
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

/// A final symlink to a FIFO must be refused, or followed without blocking.
///
/// Under the default and explicit `false` policies the link itself is the
/// rejected component; under the `true` opt-in the link is resolved and the
/// non-blocking open reaches the regular-file check. The opt-in arm is the one
/// that would hang if `O_NONBLOCK` were conditional on the policy.
#[cfg(unix)]
#[rstest]
#[case::contents_default(CONTENTS, Policy::Omitted)]
#[case::contents_false(CONTENTS, Policy::FollowFalse)]
#[case::contents_true(CONTENTS, Policy::FollowTrue)]
#[case::linecount_default(LINECOUNT, Policy::Omitted)]
#[case::linecount_false(LINECOUNT, Policy::FollowFalse)]
#[case::linecount_true(LINECOUNT, Policy::FollowTrue)]
#[case::hash_default(HASH, Policy::Omitted)]
#[case::hash_false(HASH, Policy::FollowFalse)]
#[case::hash_true(HASH, Policy::FollowTrue)]
#[case::digest_default(DIGEST, Policy::Omitted)]
#[case::digest_false(DIGEST, Policy::FollowFalse)]
#[case::digest_true(DIGEST, Policy::FollowTrue)]
fn reading_filters_reject_a_symlink_to_a_fifo(
    #[case] case: FilterCase,
    #[case] policy: Policy,
) -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    create_fifo(&root, "pipe")?;
    let link = create_symlink(&root, "pipe_link", "pipe")?;
    require_real_symlink(&root, &link)?;
    let err = rejection(
        case,
        render_case(
            case,
            &format!("pipe_link_{}", policy.suffix()),
            ReadTarget::new(&root, &link, 1024),
            &policy.template(case),
        )?,
    )?;
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "{}: should report InvalidOperation for a symlink to a FIFO but was {:?}",
        case.name,
        err.kind()
    );
    let message = normalize_fluent_isolates(&err.to_string());
    if policy.follows() {
        ensure!(
            message.contains("not a regular file"),
            "{}: the opt-in should reach the regular-file check rather than block: {message}",
            case.name
        );
    } else {
        ensure!(
            message.contains("not a regular file") || message.contains("symbolic links"),
            "{}: error should explain the rejection: {message}",
            case.name
        );
    }
    Ok(())
}
