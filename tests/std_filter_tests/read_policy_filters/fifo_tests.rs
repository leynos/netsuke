//! FIFO policy tests for the file-reading filters.
//!
//! Opening a FIFO for reading with neither a writer nor `O_NONBLOCK` blocks
//! until a writer appears, so these cases would hang rather than fail if the
//! non-blocking open were lost. They pin that the non-blocking open survives
//! every spelling of the `follow_symlinks` policy, that a FIFO is rejected by
//! the regular-file check whoever opens it, and that a final symlink to a FIFO
//! is refused under the no-follow policy and resolved without blocking under
//! the opt-in.
//!
//! The module is Unix-only: Windows names pipes through a separate object
//! namespace, and the file-reading boundary refuses a reparse point there
//! before the type of its target can matter.
use anyhow::{Result, ensure};
use minijinja::ErrorKind;
use rstest::rstest;
use test_support::fluent::normalize_fluent_isolates;

use super::{
    CONTENTS, DIGEST, FilterCase, HASH, LINECOUNT, Policy, ReadTarget, create_fifo, create_symlink,
    fallible, rejection, render_case, require_real_symlink,
};

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
///
/// The no-follow arms assert against the regular-file diagnostic rather than
/// merely accepting either wording. A resolver that followed the link would
/// reach the regular-file check, which reports "not a regular file" exactly as
/// a correct rejection of the FIFO would, so accepting that message would let
/// the dereference pass for the refusal it is meant to be.
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
            !message.contains("not a regular file"),
            "{}: the link should be refused before its target is opened; \
             the regular-file diagnostic means it was followed: {message}",
            case.name
        );
        ensure!(
            message.contains("symbolic links"),
            "{}: error should explain the symlink rejection: {message}",
            case.name
        );
    }
    Ok(())
}
