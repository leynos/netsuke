//! The traced child that asserts the read policy under the manual resolver.
//!
//! `strace` traces a whole process tree and cannot be scoped to one test, so
//! the child is an `#[ignore]`d test in this binary that the parent re-runs
//! under an injected `openat2` fault. Keeping it in its own module separates
//! the two responsibilities: this module asserts what the filters render, and
//! `resolver_fallback_tests` asserts that the resolver which rendered them had
//! actually fallen back.
//!
//! The child renders every filter under every policy and checks the documented
//! outcome, including the case that would hang rather than fail if the open
//! were allowed to block. It runs only when the parent sets
//! [`CHILD_MARKER`] on the child `Command`, so an ordinary test run of this
//! binary does nothing here.
use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use minijinja::ErrorKind;
use test_support::fluent::normalize_fluent_isolates;

use super::resolver_fallback_tests::{
    CASES, CHILD_MARKER, control_fixture_name, link_fixture_name,
};
use super::{
    FilterCase, Policy, ReadTarget, create_symlink, fallible, rejection, render_case,
    require_real_symlink, write_fixture,
};

/// Whether the parent asked this process to run as its traced child.
#[expect(
    clippy::disallowed_methods,
    reason = "the marker is set on the child Command because the trace claims the whole process; there is no injected environment in an integration-test binary"
)]
fn is_requested_child() -> bool {
    std::env::var_os(CHILD_MARKER).is_some()
}

/// The child: render every filter under every policy under the manual resolver.
///
/// Run only by `manual_resolver_enforces_the_no_follow_policy`, through
/// `strace` with `openat2` faulted. It asserts the policy itself; the parent
/// asserts that the manual path was taken.
#[test]
#[ignore = "spawned by manual_resolver_enforces_the_no_follow_policy under strace"]
fn manual_resolver_child_renders_every_policy() -> Result<()> {
    if !is_requested_child() {
        return Ok(());
    }

    let (_temp, root) = fallible::filter_workspace()?;
    for case in CASES {
        let control = write_fixture(&root, &control_fixture_name(case), b"data")?;
        let rendered = render_case(
            case,
            "resolver_control",
            ReadTarget::new(&root, &control, 1024),
            &case.template(),
        )?
        .with_context(|| format!("{}: regular-file control", case.name))?;
        ensure!(
            rendered == case.followed,
            "{}: expected {} for the regular-file control but rendered {rendered}",
            case.name,
            case.followed
        );

        for policy in Policy::ALL {
            let link = create_symlink(&root, &link_fixture_name(case, policy), "file")
                .with_context(|| format!("{}: create the {:?} link", case.name, policy))?;
            require_real_symlink(&root, &link)?;
            render_policy_case(&root, case, policy, &link)?;
        }
    }
    Ok(())
}

/// Render one symlink case under `policy` and assert the documented outcome.
fn render_policy_case(
    root: &Utf8Path,
    case: FilterCase,
    policy: Policy,
    link: &Utf8Path,
) -> Result<()> {
    let rendered = render_case(
        case,
        &format!("resolver_{}", policy.suffix()),
        ReadTarget::new(root, link, 1024),
        &policy.template(case),
    )?;

    if policy.follows() {
        let output =
            rendered.with_context(|| format!("{}: render under {:?}", case.name, policy))?;
        ensure!(
            output == case.followed,
            "{}: the opt-in should read the link target under the manual resolver; \
             expected {} but rendered {output}",
            case.name,
            case.followed
        );
        return Ok(());
    }

    let err = rejection(case, rendered)?;
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "{}: should report InvalidOperation under {:?} but was {:?}",
        case.name,
        policy,
        err.kind()
    );
    let message = normalize_fluent_isolates(&err.to_string());
    ensure!(
        message.contains("not a regular file") || message.contains("symbolic links"),
        "{}: error should explain the rejection under {:?}: {message}",
        case.name,
        policy
    );
    Ok(())
}
