//! Workspace-confinement tests for the file-reading filters.
//!
//! Every reading filter resolves its path through the workspace capability
//! configured on the stdlib environment. These cases pin both halves of that
//! contract: a path that would leave the workspace is refused, and each form
//! of path that stays inside it still renders. Both halves run through the
//! shared filter table so a filter that quietly keeps its ambient open cannot
//! pass on a sibling's behaviour.
use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use minijinja::ErrorKind;
use rstest::rstest;
use test_support::fluent::normalize_fluent_isolates;

use super::fallible::{ConfinementWorkspace, SECRET_CONTENT, confinement_workspace};
use super::{CONTENTS, DIGEST, FilterCase, HASH, LINECOUNT, ReadTarget, rejection, render_case};

/// Diagnostics that must accompany a path resolved outside the workspace.
const OUTSIDE_WORKSPACE_FRAGMENT: &str = "outside the workspace";

/// Assert that `err` is the confinement refusal for `case`.
///
/// The assertion also proves the refusal did not read the file first: the
/// secret's contents must never appear in a diagnostic.
fn assert_confined(case: FilterCase, err: &minijinja::Error) -> Result<()> {
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "{}: confinement refusal should report InvalidOperation but was {:?}: {err}",
        case.name,
        err.kind()
    );
    let message = normalize_fluent_isolates(&err.to_string());
    ensure!(
        message.contains(OUTSIDE_WORKSPACE_FRAGMENT),
        "{}: refusal should explain the workspace boundary: {message}",
        case.name
    );
    ensure!(
        !message.contains(SECRET_CONTENT),
        "{}: refusal must not disclose the outside file's contents: {message}",
        case.name
    );
    Ok(())
}

/// Render `target` through `case` and assert the workspace refusal.
fn refuse(case: FilterCase, target: ReadTarget<'_>, template: &str) -> Result<()> {
    let err = rejection(case, render_case(case, "confined", target, template)?)?;
    assert_confined(case, &err)
}

/// Report that this host cannot create the escape links, then end the case.
///
/// `Ok(None)` from the fixture means the platform cannot create a symlink at
/// all. The case says so rather than passing silently: a suite that reports
/// green while quietly skipping its subject would hide the same regression on
/// a host that can create links.
#[expect(
    clippy::print_stderr,
    reason = "test harness: an unavailable fixture must be visible in the captured test output instead of passing silently"
)]
fn skip_without_links(case: FilterCase) {
    eprintln!(
        "skipped: {} — this host cannot create the confinement link fixtures",
        case.name
    );
}

#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn reading_filters_reject_an_absolute_path_outside_the_workspace(
    #[case] case: FilterCase,
) -> Result<()> {
    let ws = confinement_workspace()?;
    refuse(
        case,
        ReadTarget::new(&ws.root, &ws.secret, 1024),
        &case.template(),
    )
}

#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn reading_filters_reject_a_parent_relative_escape(#[case] case: FilterCase) -> Result<()> {
    let ws = confinement_workspace()?;
    let escaping = Utf8Path::new("../outside/secret");
    refuse(
        case,
        ReadTarget::new(&ws.root, escaping, 1024),
        &case.template(),
    )
}

#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn reading_filters_reject_an_absolute_parent_escape(#[case] case: FilterCase) -> Result<()> {
    let ws = confinement_workspace()?;
    let escaping = ws.root.join("../outside/secret");
    refuse(
        case,
        ReadTarget::new(&ws.root, &escaping, 1024),
        &case.template(),
    )
}

/// An intermediate directory link must not smuggle the parent open out of the
/// workspace.
///
/// `workspace/escape` is a real link to `../outside`. The final component is
/// therefore an ordinary file inside the *linked* directory, so the
/// final-component symlink policy never sees a link; only resolving the parent
/// through the capability stops the read.
#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn reading_filters_reject_an_intermediate_link_that_escapes(
    #[case] case: FilterCase,
) -> Result<()> {
    let ws = confinement_workspace()?;
    let Some(links) = ws.links.as_ref() else {
        skip_without_links(case);
        return Ok(());
    };
    refuse(
        case,
        ReadTarget::new(&ws.root, &links.escape.join("secret"), 1024),
        &case.template(),
    )
}

/// The follow-symlinks opt-in waives the final-component refusal, not the
/// workspace boundary.
///
/// `workspace/out` is a real link to `../outside/secret`, so this is the case
/// where the opt-in would otherwise open an outside file by design.
#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn reading_filters_reject_a_final_link_that_escapes_when_following(
    #[case] case: FilterCase,
) -> Result<()> {
    let ws = confinement_workspace()?;
    let Some(links) = ws.links.as_ref() else {
        skip_without_links(case);
        return Ok(());
    };
    refuse(
        case,
        ReadTarget::new(&ws.root, &links.out, 1024),
        &case.template_with("follow_symlinks=true"),
    )
}

/// One inside-working-directory spelling, with the case field holding its
/// expected render.
struct InsideTarget {
    /// Path as the template would write it.
    path: Utf8PathBuf,
    /// The `FilterCase` field naming the expected render.
    expected: fn(FilterCase) -> &'static str,
}

/// Every path form the reading filters must still accept.
///
/// Relative paths are resolved beneath the workspace root rather than the
/// process working directory, and absolute paths beneath the configured root
/// path are accepted in their normalized relative form.
fn inside_targets(ws: &ConfinementWorkspace) -> Vec<InsideTarget> {
    let mut targets = vec![
        InsideTarget {
            path: Utf8PathBuf::from("file"),
            expected: |case| case.followed,
        },
        InsideTarget {
            path: Utf8PathBuf::from("sub/../file"),
            expected: |case| case.followed,
        },
        InsideTarget {
            path: ws.root.join("file"),
            expected: |case| case.followed,
        },
        InsideTarget {
            path: ws.root.join("sub/../file"),
            expected: |case| case.followed,
        },
        InsideTarget {
            path: Utf8PathBuf::from("sub/inner.txt"),
            expected: |case| case.inner,
        },
        InsideTarget {
            path: ws.root.join("sub/inner.txt"),
            expected: |case| case.inner,
        },
    ];
    // The alias link is the inside-staying counterpart of the escaping link;
    // a host that cannot create it has no link behaviour to accept.
    if let Some(links) = ws.links.as_ref() {
        targets.push(InsideTarget {
            path: links.alias.join("inner.txt"),
            expected: |case| case.inner,
        });
    }
    targets
}

#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn reading_filters_accept_paths_inside_the_workspace(#[case] case: FilterCase) -> Result<()> {
    let ws = confinement_workspace()?;
    for target in inside_targets(&ws) {
        let expected = (target.expected)(case);
        let rendered = render_case(
            case,
            "inside",
            ReadTarget::new(&ws.root, &target.path, 1024),
            &case.template(),
        )?
        .with_context(|| format!("{}: read inside path {}", case.name, target.path))?;
        ensure!(
            rendered == expected,
            "{}: reading {} should render {expected} but rendered {rendered}",
            case.name,
            target.path
        );
    }
    Ok(())
}

/// A Windows verbatim path must not bypass the lexical workspace check.
///
/// `\\?\` paths opt out of Win32 normalization, so a lexical comparison that
/// treated one as equal to the workspace root would let the remainder contain
/// `..` unchecked. The verbatim spelling is not rewritten into a relative path,
/// so it is refused as outside the workspace.
#[cfg(windows)]
#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn reading_filters_reject_a_verbatim_absolute_path(#[case] case: FilterCase) -> Result<()> {
    let ws = confinement_workspace()?;
    let verbatim = Utf8PathBuf::from(format!(r"\\?\{}", ws.secret.as_str()));
    refuse(
        case,
        ReadTarget::new(&ws.root, &verbatim, 1024),
        &case.template(),
    )
}

/// The confinement fixture itself must be sound: the outside file exists and
/// holds the sentinel an over-permissive read would render.
///
/// Without this probe the rejection cases could pass because the secret was
/// missing, empty, or unreadable, rather than because the filters confined it.
#[rstest]
fn confinement_fixture_holds_the_outside_secret() -> Result<()> {
    let ws = confinement_workspace()?;
    ensure!(
        ws.secret.starts_with(&ws.outside_dir),
        "the secret must live outside the workspace: {}",
        ws.secret
    );
    let outside =
        cap_std::fs_utf8::Dir::open_ambient_dir(&ws.outside_dir, cap_std::ambient_authority())
            .context("open the outside directory to verify the fixture")?;
    let contents = outside
        .read_to_string("secret")
        .context("read the outside secret fixture")?;
    ensure!(
        contents == SECRET_CONTENT,
        "the outside fixture must hold the sentinel, but holds {contents:?}"
    );
    Ok(())
}
