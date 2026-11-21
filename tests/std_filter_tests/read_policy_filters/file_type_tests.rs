//! Symlink and file-type policy tests for the file-reading filters.
//!
//! The default policy opens the final path component without following
//! symlinks and rejects anything that is not a regular file; the
//! `follow_symlinks` opt-in resolves a link. Both clauses are asserted for
//! every entry point through the shared filter table. The FIFO cases live in
//! `fifo_tests`, and the Windows reparse-point case in this module's
//! `#[cfg(windows)]` half.
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

/// The default and explicit `false` policies must both refuse a final symlink.
///
/// The two spellings are the same policy, so one table asserts both: a caller
/// that omits `follow_symlinks` and a caller that writes `false` must reach the
/// same refusal.
///
/// Native Linux resolves this through `openat2`, where the raw `O_NOFOLLOW`
/// flag already refuses the link. The manual resolver consults cap-std's
/// logical policy instead, so a caller that omits `FollowSymlinks::No` reads
/// the target on macOS and on every fallback path; `resolver_fallback_tests`
/// forces that resolver and provides the pre-fix failure evidence.
#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn reading_filters_reject_symlinks(
    #[case] case: FilterCase,
    #[values(Policy::Omitted, Policy::FollowFalse)] policy: Policy,
) -> Result<()> {
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
            &format!("symlink_{}", policy.suffix()),
            ReadTarget::new(&root, &link, 1024),
            &policy.template(case),
        )?,
    )?;
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "{}: should report InvalidOperation for a symlink under {policy:?} but was {:?}",
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

/// The Windows default policy must refuse a junction through the same open
/// that yields the handle.
///
/// A junction is a directory-shaped reparse point: reading it back as text
/// renders the target's file names, so a policy that let the open traverse it
/// would silently expose a directory the caller never asked for. This case is
/// the Windows half of the same-handle guarantee, where the decision comes
/// from the opened handle's reparse-point attribute rather than from a
/// pre-open `symlink_metadata` lookup that a concurrent replace could defeat.
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
