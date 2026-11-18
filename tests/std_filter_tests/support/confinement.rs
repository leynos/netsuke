//! A fixture tree pairing a workspace with files it must not reach.
//!
//! The workspace is a sibling of the outside directory rather than a child of
//! it, so a parent-relative request has somewhere real to land. The escape
//! links let a test prove that the resolver, not an accident of layout,
//! refuses the read.

#[cfg(windows)]
use super::file_types::{create_junction, is_symlink_privilege_error};
use anyhow::{Context, Result, anyhow};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use tempfile::tempdir;

/// A temporary tree pairing a workspace with files it must not reach.
///
/// The workspace is a *sibling* of the outside directory rather than a
/// child of it, so `..` from the workspace lands in the temporary tree
/// root and a parent-relative escape has somewhere real to go. The layout
/// is:
///
/// ```text
/// <temp>/
///   workspace/
///     file                (data)
///     lines.txt           (one\ntwo\nthree\n)
///     sub/file            (linked\nparent)
///     sub/deep/inner.txt  (inner)
///     escape -> ../outside            (Unix only)
///     alias  -> sub/deep              (Unix only)
///     out    -> ../outside/secret     (Unix only)
///     sub/upto -> ../file             (Unix only)
///     jescape -> ../outside           (Windows only, a junction)
///   outside/
///     secret          (SOLID_SECRET_CONTENT)
/// ```
///
/// The absolute-target link `absalias -> <root>/sub` is not part of this tree:
/// [`ConfinementWorkspace::create_absolute_alias`] creates it for the one case
/// that reads it, so a host that cannot create that link fails only that case.
pub(crate) struct ConfinementWorkspace {
    /// Keeps the temporary tree alive for the life of the test.
    _temp: tempfile::TempDir,
    /// The configured workspace root.
    pub(crate) root: Utf8PathBuf,
    /// The outside directory the workspace must not reach.
    pub(crate) outside_dir: Utf8PathBuf,
    /// The outside file whose contents must never be read.
    pub(crate) secret: Utf8PathBuf,
    /// The escape links, when this host can create symlinks.
    pub(crate) links: Option<EscapeLinks>,
    /// An intermediate junction to the outside directory, when one could be
    /// created. Unlike a symlink it needs no privilege on Windows, so this is
    /// the intermediate reparse point every Windows host can be tested with.
    ///
    /// Only Windows has junctions, and only the Windows case reads this field,
    /// so it is absent elsewhere rather than dead weight in every test build.
    #[cfg(windows)]
    pub(crate) junction: Option<Utf8PathBuf>,
}

/// Real symlinks that a confined read must not follow out of the
/// workspace.
///
/// Each link's target is relative, so the fixtures exercise the resolver
/// rather than a platform refusal of absolute link targets. The one
/// absolute-target link is created on demand by
/// [`ConfinementWorkspace::create_absolute_alias`].
pub(crate) struct EscapeLinks {
    /// `workspace/escape` -> `../outside`, an intermediate directory link.
    pub(crate) escape: Utf8PathBuf,
    /// `workspace/alias` -> `sub/deep`, an intermediate link that stays
    /// inside and whose target is deep enough that `alias/..` differs from
    /// the link's lexical parent.
    pub(crate) alias: Utf8PathBuf,
    /// `workspace/out` -> `../outside/secret`, a final component link.
    pub(crate) out: Utf8PathBuf,
    /// `workspace/sub/upto` -> `../file`, a final component link whose target
    /// stays inside the workspace but climbs above the file's own parent
    /// directory. Only the Unix fixture builds it.
    #[cfg(unix)]
    pub(crate) climbing: Utf8PathBuf,
}

/// Contents of the file outside the workspace.
///
/// Deliberately unlike any path fragment: an assertion that a rejection
/// does not disclose the secret must not pass merely because the
/// diagnostic quotes the path `outside/secret`.
pub(crate) const SECRET_CONTENT: &str = "TOP-SECRET-OUTSIDE-PAYLOAD-968";

/// Build the confinement tree described on [`ConfinementWorkspace`].
///
/// `links` is `None` when this host cannot create symlinks at all; on
/// Windows that is the environmental
/// `ERROR_PRIVILEGE_NOT_HELD` condition, mirroring
/// [`file_symlink_fixture`]. Every other failure is a setup fault.
///
/// # Errors
///
/// Returns the setup error when a directory or fixture file cannot be
/// written, or when symlink creation fails for a reason other than the
/// documented unavailability.
pub(crate) fn confinement_workspace() -> Result<ConfinementWorkspace> {
    let temp = tempdir().context("create confinement workspace tree")?;
    let base = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).map_err(|path| {
        anyhow!(
            "confinement workspace path is not valid UTF-8: {}",
            path.display()
        )
    })?;
    let root = base.join("workspace");
    let outside_dir = base.join("outside");
    let secret = outside_dir.join("secret");

    let base_dir =
        Dir::open_ambient_dir(&base, ambient_authority()).context("open confinement tree root")?;
    base_dir
        .create_dir("workspace")
        .context("create confinement workspace")?;
    base_dir
        .create_dir("outside")
        .context("create confinement outside directory")?;
    Dir::open_ambient_dir(&outside_dir, ambient_authority())
        .context("open confinement outside directory")?
        .write("secret", SECRET_CONTENT.as_bytes())
        .context("write the outside secret")?;

    let handle = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open confinement workspace directory")?;
    handle
        .write("file", b"data")
        .context("write confinement fixture 'file'")?;
    handle
        .write("lines.txt", b"one\ntwo\nthree\n")
        .context("write confinement fixture 'lines.txt'")?;
    handle
        .create_dir_all("sub/deep")
        .context("create confinement fixture directory 'sub/deep'")?;
    handle
        .write("sub/inner.txt", b"inner")
        .context("write confinement fixture 'sub/inner.txt'")?;
    handle
        .write("sub/deep/inner.txt", b"inner")
        .context("write confinement fixture 'sub/deep/inner.txt'")?;
    // The alias target is deliberately two levels deep, so `alias/../file`
    // and a resolver that folds `..` before opening disagree: the filesystem
    // names `sub/file`, while a lexical fold names the root's own `file`.
    // Its contents differ from the root file in length, line count, and hash,
    // so all four reading filters can tell which of the two was read.
    handle
        .write("sub/file", b"linked\nparent")
        .context("write confinement fixture 'sub/file'")?;

    let links = create_escape_links(&root, &handle)?;
    // Build the junction before the struct literal: a field initialiser is
    // evaluated in written order, so borrowing `root` and `outside_dir` from
    // inside the literal would read them after they had been moved.
    #[cfg(windows)]
    let junction = create_junction_escape(&root, &outside_dir)?;
    Ok(ConfinementWorkspace {
        _temp: temp,
        root,
        outside_dir,
        secret,
        links,
        #[cfg(windows)]
        junction,
    })
}

/// Create an intermediate junction from the workspace to the outside
/// directory.
///
/// Only Windows needs one: a symlink there requires a privilege that most CI
/// hosts lack, so without a junction the intermediate-link case would be
/// skipped on the platform whose reparse-point handling differs most. The
/// junction is named beside the symlink fixture so both can exist at once.
///
/// # Errors
///
/// Returns the setup error when the junction could not be created for a
/// reason other than `cmd.exe` being unavailable.
#[cfg(windows)]
fn create_junction_escape(root: &Utf8Path, outside: &Utf8Path) -> Result<Option<Utf8PathBuf>> {
    let link = root.join("jescape");
    Ok(create_junction(&link, outside)?.map(|()| link))
}

/// Create the escape links inside `root`.
///
/// Unix creates them through the capability handle; Windows goes through
/// `std::os::windows::fs` because that is where the platform reports
/// `ERROR_PRIVILEGE_NOT_HELD`, the one condition that makes the fixture
/// unavailable rather than faulty.
#[cfg(unix)]
fn create_escape_links(root: &Utf8Path, handle: &Dir) -> Result<Option<EscapeLinks>> {
    handle
        .symlink("../outside", "escape")
        .context("create escape link 'escape' -> '../outside'")?;
    handle
        .symlink("sub/deep", "alias")
        .context("create escape link 'alias' -> 'sub/deep'")?;
    handle
        .symlink("../outside/secret", "out")
        .context("create escape link 'out' -> '../outside/secret'")?;
    handle
        .symlink("../file", "sub/upto")
        .context("create climbing link 'sub/upto' -> '../file'")?;
    Ok(Some(EscapeLinks::under(root)))
}

/// Create the three escape links inside `root` on Windows.
///
/// `Ok(None)` means this host cannot create symlinks for want of
/// `SeCreateSymbolicLinkPrivilege` and Developer Mode.
#[cfg(windows)]
fn create_escape_links(root: &Utf8Path, _handle: &Dir) -> Result<Option<EscapeLinks>> {
    let links = EscapeLinks::under(root);
    let attempts = [
        std::os::windows::fs::symlink_dir("../outside", &links.escape),
        std::os::windows::fs::symlink_dir("sub/deep", &links.alias),
        std::os::windows::fs::symlink_file("../outside/secret", &links.out),
    ];
    for attempt in attempts {
        match attempt {
            Ok(()) => {}
            Err(err) if is_symlink_privilege_error(&err) => return Ok(None),
            Err(err) => return Err(err).context("create confinement escape symlinks"),
        }
    }
    Ok(Some(links))
}

/// Report that this host cannot create symlinks while creating the
/// confinement tree.
#[cfg(not(any(unix, windows)))]
fn create_escape_links(root: &Utf8Path, _handle: &Dir) -> Result<Option<EscapeLinks>> {
    let _ = root;
    Ok(None)
}

impl EscapeLinks {
    /// Name the links beneath `root`.
    fn under(root: &Utf8Path) -> Self {
        Self {
            escape: root.join("escape"),
            alias: root.join("alias"),
            out: root.join("out"),
            #[cfg(unix)]
            climbing: root.join("sub/upto"),
        }
    }
}

impl ConfinementWorkspace {
    /// Create the absolute-target link the absolute-target case reads through,
    /// and return its path.
    ///
    /// Kept out of the shared tree so this one link cannot fail the cases that
    /// never read it: a filesystem that refuses the target, or a temporary path
    /// deep enough to make it too long, then fails only the case that needs it.
    /// `Dir::symlink` refuses an absolute target by design, so the link is
    /// written with `Dir::symlink_contents`, which stores the target text
    /// verbatim while still resolving the link entry through the workspace
    /// capability rather than its ambient path.
    ///
    /// # Errors
    ///
    /// Returns the setup error when the workspace cannot be reopened or the
    /// link cannot be created.
    #[cfg(unix)]
    pub(crate) fn create_absolute_alias(&self) -> Result<Utf8PathBuf> {
        let handle = Dir::open_ambient_dir(&self.root, ambient_authority())
            .context("open confinement workspace directory")?;
        handle
            .symlink_contents(self.root.join("sub"), "absalias")
            .context("create absolute-target link 'absalias' -> '<root>/sub'")?;
        Ok(self.root.join("absalias"))
    }
}
