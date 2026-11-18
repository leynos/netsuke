//! Reading-filter refusals that are reached by following a link.
//!
//! A link moves the resolver's anchor: an intermediate link changes which
//! directory a later component is looked up in, and a final link is resolved
//! from the capability opened for the file's own parent. Each case pins one of
//! those paths, so a filter that stops resolving through the capability — or
//! that folds `..` before the filesystem would — fails on its own.
use anyhow::Result;
use camino::Utf8PathBuf;
use rstest::rstest;

use super::super::fallible::{ConfinementWorkspace, confinement_workspace};
use super::{CONTENTS, DIGEST, FilterCase, HASH, LINECOUNT, ReadTarget, refuse};

// The alias case is the only one that renders rather than refuses, and only
// the Unix fixture builds the link it reads.
#[cfg(unix)]
use super::render_case;
#[cfg(unix)]
use anyhow::{Context, ensure};

/// One link fixture a refusal case reads through.
///
/// Every case needs the same steps — build the tree, take the link this host
/// could create, and refuse the request it names — so the targets are named
/// here instead of being spelled out in each body. A target whose fixture this
/// host could not create reports its own case as skipped rather than passing
/// silently.
#[derive(Clone, Copy)]
enum LinkTarget {
    /// `escape/secret`, behind the intermediate link to the outside directory.
    Intermediate,
    /// `out`, the final component link to the outside file, read with
    /// `follow_symlinks=true`.
    Final,
    /// `sub/upto`, a final component link whose target is inside the workspace
    /// but climbs above the file's own parent directory.
    #[cfg(unix)]
    Climbing,
    /// `absalias/file`, behind an intermediate link whose target is the
    /// absolute spelling of a directory inside the workspace.
    #[cfg(unix)]
    AbsoluteAlias,
    /// `jescape/secret`, behind the Windows junction to the outside directory.
    #[cfg(windows)]
    Junction,
}

impl LinkTarget {
    /// The request this target names and the template that reads it, or `None`
    /// when this host could not create the link it needs.
    fn request(self, ws: &ConfinementWorkspace, case: FilterCase) -> Option<(Utf8PathBuf, String)> {
        match self {
            Self::Intermediate => {
                let links = ws.links.as_ref()?;
                Some((links.escape.join("secret"), case.template()))
            }
            Self::Final => {
                let links = ws.links.as_ref()?;
                Some((
                    links.out.clone(),
                    case.template_with("follow_symlinks=true"),
                ))
            }
            #[cfg(unix)]
            Self::Climbing => {
                let links = ws.links.as_ref()?;
                Some((
                    links.climbing.clone(),
                    case.template_with("follow_symlinks=true"),
                ))
            }
            #[cfg(unix)]
            Self::AbsoluteAlias => {
                let links = ws.links.as_ref()?;
                Some((links.absolute_alias.join("file"), case.template()))
            }
            #[cfg(windows)]
            Self::Junction => {
                let junction = ws.junction.as_ref()?;
                Some((junction.join("secret"), case.template()))
            }
        }
    }

    /// Report `case` as skipped, naming the fixture this host could not create.
    fn report_skip(self, case: FilterCase) {
        match self {
            #[cfg(windows)]
            Self::Junction => super::skip_without_cmd(case),
            _ => super::skip_without_links(case),
        }
    }
}

/// Refuse the request `target` names, skipping the case when its link is
/// unavailable on this host.
fn refuse_through(case: FilterCase, target: LinkTarget) -> Result<()> {
    let ws = confinement_workspace()?;
    let Some((requested, template)) = target.request(&ws, case) else {
        target.report_skip(case);
        return Ok(());
    };
    refuse(case, ReadTarget::new(&ws.root, &requested, 1024), &template)
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
    refuse_through(case, LinkTarget::Intermediate)
}

/// An intermediate link is followed only when its target is written as a
/// relative path, even when the target is inside the workspace.
///
/// `workspace/absalias` points at the absolute spelling of `workspace/sub`, so
/// the file it names is inside the workspace and `absalias/file` would read the
/// same bytes as `sub/file`. cap-std refuses to follow an absolute link target
/// all the same: it cannot tell where such a target points without leaving the
/// capability it was given, so it reports an escape. The case pins that rule so
/// the guide's statement about relative link targets stays true.
#[cfg(unix)]
#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn reading_filters_refuse_an_intermediate_link_with_an_absolute_target(
    #[case] case: FilterCase,
) -> Result<()> {
    refuse_through(case, LinkTarget::AbsoluteAlias)
}

/// A `..` behind an intermediate link is the filesystem's `..`, not a lexical
/// fold of the link's own name.
///
/// `workspace/alias` points at `sub/deep`, so `alias/../file` names `sub/file`.
/// A resolver that removed the `..` before opening would instead read the
/// workspace root's `file`, silently changing which file a template selects.
/// The two files differ in length, line count, and hash, so every filter's
/// rendered value says which one was read.
#[cfg(unix)]
#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn reading_filters_resolve_a_parent_component_after_an_intermediate_link(
    #[case] case: FilterCase,
) -> Result<()> {
    let ws = confinement_workspace()?;
    let Some(links) = ws.links.as_ref() else {
        super::skip_without_links(case);
        return Ok(());
    };
    let requested = links.alias.join("../file");
    let rendered = render_case(
        case,
        "alias-parent",
        ReadTarget::new(&ws.root, &requested, 1024),
        &case.template(),
    )?
    .with_context(|| format!("{}: read {requested}", case.name))?;
    ensure!(
        rendered == case.linked,
        "{}: {requested} should read sub/file ({}), but rendered {rendered}",
        case.name,
        case.linked
    );
    Ok(())
}

/// An intermediate junction must not carry the parent open out of the
/// workspace.
///
/// Windows cannot create a symlink without a privilege most CI hosts lack, but
/// a junction needs none. The junction points at the outside directory, so the
/// final component is an ordinary file inside the *linked* directory and only
/// the intermediate-component check can stop the read.
#[cfg(windows)]
#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn reading_filters_reject_an_intermediate_junction_that_escapes(
    #[case] case: FilterCase,
) -> Result<()> {
    refuse_through(case, LinkTarget::Junction)
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
    refuse_through(case, LinkTarget::Final)
}

/// A relative final link that climbs above the file's own parent directory is
/// refused even though its target is inside the workspace.
///
/// The link target is resolved from the capability opened for the file's
/// parent, not from the workspace root, so `sub/upto -> ../file` asks to leave
/// the directory `sub` and is refused. Templates that need such a link must
/// name the target directly, or place the link where its target stays beneath
/// the same parent.
#[cfg(unix)]
#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn reading_filters_refuse_a_final_link_that_climbs_above_its_parent(
    #[case] case: FilterCase,
) -> Result<()> {
    refuse_through(case, LinkTarget::Climbing)
}
