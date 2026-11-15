//! Tests for workspace-confined path resolution.
//!
//! Resolution is the decision that makes the reading filters' capability
//! meaningful, so the cases cover each spelling a template can supply and the
//! lexical edges around the root. The capability-escape cases drive a real
//! link, because cap-std's escape error is the seam the filter diagnostics
//! depend on.

use anyhow::{Context, Result, anyhow, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::ErrorKind;
use rstest::rstest;
use std::sync::Arc;
use test_support::{localizer_test_lock, set_en_localizer};

use super::{ReadingRoot, is_capability_escape};

/// A reading root bound to `directory` with its absolute spelling recorded.
fn root_at(directory: &Utf8Path) -> Result<ReadingRoot> {
    root_recorded_at(directory, directory)
}

/// A reading root bound to `directory` but recorded under `recorded`.
///
/// Lexical resolution never opens through the recorded spelling, so a case can
/// name a root beneath the scratch directory without creating it.
fn root_recorded_at(directory: &Utf8Path, recorded: &Utf8Path) -> Result<ReadingRoot> {
    let handle = Dir::open_ambient_dir(directory, ambient_authority())
        .with_context(|| format!("open reading root {directory}"))?;
    Ok(ReadingRoot::new(
        Arc::new(handle),
        Some(recorded.to_path_buf()),
    ))
}

/// An absolute spelling of the workspace root beneath the scratch directory.
///
/// Deriving the spelling from the scratch path keeps the requests in the
/// absolute-path cases genuinely absolute everywhere. A hardcoded POSIX
/// spelling would not be: Windows requires a drive prefix before a path counts
/// as absolute, so the request would fall through to the relative branch and
/// the case would prove nothing.
fn absolute_root(path: &Utf8Path) -> Utf8PathBuf {
    path.join("work")
}

/// Open a scratch directory that every resolution case can name.
fn scratch() -> Result<(tempfile::TempDir, Utf8PathBuf, ReadingRoot)> {
    let temp = tempfile::tempdir().context("create reading-root scratch directory")?;
    let path = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow!("scratch path is not valid UTF-8: {path:?}"))?;
    let root = root_at(&path)?;
    Ok((temp, path, root))
}

/// The resolved relative path for `requested`, or `None` when refused.
fn resolved(root: &ReadingRoot, requested: &str) -> Option<Utf8PathBuf> {
    root.resolve(Utf8Path::new(requested)).ok()
}

#[rstest]
#[case::bare_name("file", "file")]
#[case::nested("sub/inner.txt", "sub/inner.txt")]
#[case::current_directory("./file", "file")]
#[case::inside_parent("sub/../file", "sub/../file")]
#[case::redundant_separators("sub//inner.txt", "sub/inner.txt")]
fn relative_requests_keep_every_component_the_filesystem_must_interpret(
    #[case] requested: &str,
    #[case] expected: &str,
) -> Result<()> {
    let (_temp, _path, root) = scratch()?;
    let actual = resolved(&root, requested).ok_or_else(|| {
        anyhow!("relative request {requested:?} should resolve inside the workspace")
    })?;
    ensure!(
        actual == Utf8Path::new(expected),
        "{requested:?} should resolve to {expected:?} but resolved {actual:?}"
    );
    Ok(())
}

#[rstest]
#[case::leading_parent("../outside/secret")]
#[case::only_parent("..")]
#[case::current_then_parent("./../outside/secret")]
fn requests_that_start_above_the_root_are_refused(#[case] requested: &str) -> Result<()> {
    let (_temp, _path, root) = scratch()?;
    let err = root
        .resolve(Utf8Path::new(requested))
        .expect_err("a request starting above the root must be refused");
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "{requested:?}: should report InvalidOperation but was {:?}",
        err.kind()
    );
    Ok(())
}

/// A request that climbs above the root only after naming an entry is left to
/// the capability, which holds the directory handles and can tell where the
/// climb actually lands.
///
/// `sub` must exist for the case to exercise the climb: a missing entry would
/// fail with `NotFound` before the second `..` was ever interpreted, and the
/// case would pass for the wrong reason.
#[rstest]
#[case::parent_above_root("sub/../../outside/secret")]
fn requests_that_climb_above_the_root_are_refused_by_the_capability(
    #[case] requested: &str,
) -> Result<()> {
    let (_temp, path, root) = scratch()?;
    Dir::open_ambient_dir(&path, ambient_authority())
        .context("open the scratch directory to create the climb fixture")?
        .create_dir("sub")
        .context("create the directory the climb starts from")?;
    let relative = resolved(&root, requested)
        .ok_or_else(|| anyhow!("{requested:?} should reach the capability open"))?;
    let err = root
        .open_parent(&relative)
        .expect_err("the capability must refuse a climb above the root");
    ensure!(
        is_capability_escape(&err),
        "{requested:?}: should report a capability escape but reported {err}"
    );
    Ok(())
}

/// `..` after a link names the directory the link points at, not the link's
/// own lexical parent.
///
/// The two readings differ only when the link's target is more than one level
/// deep: `alias -> sub/deep` makes `alias/../file` the file in `sub`, while a
/// resolver that folded `..` before opening would look for it at the root.
#[cfg(unix)]
#[rstest]
fn parent_components_follow_the_directory_a_link_names() -> Result<()> {
    let temp = tempfile::tempdir().context("create the link semantics fixture")?;
    let base = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow!("fixture path is not valid UTF-8: {path:?}"))?;
    let workspace = base.join("workspace");
    Dir::open_ambient_dir(&base, ambient_authority())
        .context("open the link semantics tree")?
        .create_dir("workspace")
        .context("create the link semantics workspace")?;
    let handle = Dir::open_ambient_dir(&workspace, ambient_authority())
        .context("open the link semantics fixture")?;
    handle
        .create_dir_all("sub/deep")
        .context("create the link target directory")?;
    handle
        .write("sub/file", b"chosen through the link")
        .context("write the file beside the link target")?;
    handle
        .symlink("sub/deep", "alias")
        .context("create the directory link")?;

    let root = root_at(&workspace)?;
    let relative = root
        .resolve(Utf8Path::new("alias/../file"))
        .context("resolve through the link's parent")?;
    let parent = root
        .open_parent(&relative)
        .context("open the link's parent directory")?;
    let contents = parent
        .read_to_string("file")
        .context("read the file the link's parent names")?;
    ensure!(
        contents == "chosen through the link",
        "alias/../file should open sub/file, but read {contents:?}"
    );
    Ok(())
}

/// A `..` behind a component that does not exist fails on Unix, where the
/// filesystem resolves `..` after looking the component up.
///
/// Windows folds `..` lexically before it looks anything up, so there the same
/// request names the file at the root; the companion case below pins that
/// platform's rule rather than pretending it matches Unix.
#[cfg(unix)]
#[rstest]
fn parent_components_through_a_missing_directory_are_refused() -> Result<()> {
    let (_temp, _path, root) = scratch()?;
    let relative = root
        .resolve(Utf8Path::new("missing/../file"))
        .context("resolve through the missing directory")?;
    let err = root
        .open_parent(&relative)
        .expect_err("a missing intermediate directory must fail the open");
    ensure!(
        err.kind() == std::io::ErrorKind::NotFound,
        "the missing directory should report NotFound, but reported {err}"
    );
    Ok(())
}

/// Windows resolves `..` before any lookup, so the missing directory is never
/// consulted and the request names the root's own file.
#[cfg(windows)]
#[rstest]
fn parent_components_through_a_missing_directory_fold_first() -> Result<()> {
    let temp = tempfile::tempdir().context("create the lexical fold fixture")?;
    let path = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow!("fixture path is not valid UTF-8: {path:?}"))?;
    let handle = Dir::open_ambient_dir(&path, ambient_authority())
        .context("open the lexical fold fixture")?;
    handle
        .write("file", b"folded")
        .context("write the fixture file")?;

    let root = root_at(&path)?;
    let relative = root
        .resolve(Utf8Path::new("missing/../file"))
        .context("resolve through the missing directory")?;
    let parent = root
        .open_parent(&relative)
        .context("open the folded parent")?;
    let contents = parent
        .read_to_string("file")
        .context("read the folded file")?;
    ensure!(
        contents == "folded",
        "Windows should fold missing/.. away, but read {contents:?}"
    );
    Ok(())
}

#[rstest]
fn absolute_requests_beneath_the_recorded_root_are_accepted() -> Result<()> {
    let (_temp, path, _) = scratch()?;
    let recorded = absolute_root(&path);
    let root = root_recorded_at(&path, &recorded)?;
    ensure!(
        resolved(&root, recorded.join("file").as_str()).as_deref() == Some(Utf8Path::new("file")),
        "an absolute path beneath the root should resolve relatively"
    );
    ensure!(
        resolved(&root, recorded.join("sub/../file").as_str()).as_deref()
            == Some(Utf8Path::new("sub/../file")),
        "an absolute path should keep the components the filesystem interprets"
    );
    Ok(())
}

#[rstest]
#[case::outside("other/secret")]
#[case::sibling_prefix("workspace/file")]
#[case::escapes_root("work/../other/secret")]
fn absolute_requests_outside_the_recorded_root_are_refused(#[case] suffix: &str) -> Result<()> {
    let (_temp, path, _) = scratch()?;
    let root = root_recorded_at(&path, &absolute_root(&path))?;
    let requested = path.join(suffix);
    ensure!(
        resolved(&root, requested.as_str()).is_none(),
        "{requested:?} must be refused as outside the workspace"
    );
    Ok(())
}

/// Without an absolute spelling there is no safe way to recognize an absolute
/// request, so it is refused rather than guessed at.
#[rstest]
fn absolute_requests_need_a_recorded_root_path() -> Result<()> {
    let (_temp, path, _) = scratch()?;
    let handle = Dir::open_ambient_dir(&path, ambient_authority())
        .context("open the scratch directory as a capability")?;
    let root = ReadingRoot::new(Arc::new(handle), None);
    let requested = path.join("file");
    ensure!(
        root.resolve(&requested).is_err(),
        "an absolute request without a recorded root path must be refused"
    );
    Ok(())
}

/// The refusal renders the English workspace diagnostic, not the untranslated
/// key or a leaked platform error.
///
/// The localizer is pinned for the assertion because the fallback for a missing
/// catalogue is the key itself, which also contains the word `workspace`.
/// Accepting either spelling would let the test pass without proving that the
/// message a user sees is the localized one.
#[rstest]
fn the_refusal_is_a_localized_workspace_diagnostic() -> Result<()> {
    let _lock = localizer_test_lock().map_err(|error| anyhow!("{error}"))?;
    let _localizer = set_en_localizer();
    let (_temp, path, _) = scratch()?;
    let root = root_recorded_at(&path, &absolute_root(&path))?;
    let err = root
        .resolve(&path.join("other/secret"))
        .expect_err("an outside path must be refused");
    let message = err.to_string();
    ensure!(
        message.contains("is outside the workspace"),
        "the diagnostic should be the localized workspace message: {message}"
    );
    Ok(())
}

/// The platform's access-denied code, which `std::io` maps to
/// `PermissionDenied`.
///
/// Raw code 13 is an access denial only on Unix; Windows assigns it to
/// `ERROR_INVALID_DATA`, so a shared constant would leave the negative case
/// below asserting nothing on that platform.
#[cfg(windows)]
const ACCESS_DENIED: i32 = 5;
/// The platform's access-denied code, which `std::io` maps to
/// `PermissionDenied`.
#[cfg(not(windows))]
const ACCESS_DENIED: i32 = 13;

/// A capability escape is recognized, while an ordinary permission denial is
/// not.
///
/// cap-std reports an escape as a `PermissionDenied` with no operating-system
/// code; an ACL denial carries the platform's code and must keep the ordinary
/// I/O wording instead of being relabelled as a workspace escape.
#[rstest]
fn capability_escapes_are_distinguished_from_permission_denials() {
    let escape = std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "a path led outside of the filesystem",
    );
    assert!(
        is_capability_escape(&escape),
        "an escape error should be recognized"
    );

    let denied = std::io::Error::from_raw_os_error(ACCESS_DENIED);
    assert_eq!(
        denied.kind(),
        std::io::ErrorKind::PermissionDenied,
        "the fixture must model an OS-coded permission denial"
    );
    assert!(
        !is_capability_escape(&denied),
        "an OS-coded permission denial is not a capability escape"
    );
}

/// A real intermediate link that resolves outside the capability surfaces the
/// escape error the filters translate into their workspace diagnostic.
#[cfg(unix)]
#[rstest]
fn opening_an_escaping_parent_reports_a_capability_escape() -> Result<()> {
    let temp = tempfile::tempdir().context("create the escape fixture tree")?;
    let base = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow!("escape fixture path is not valid UTF-8: {path:?}"))?;
    let workspace = base.join("workspace");
    let outside = base.join("outside");
    let handle = Dir::open_ambient_dir(&base, ambient_authority())
        .context("open the escape fixture tree")?;
    handle.create_dir("workspace").context("create workspace")?;
    handle.create_dir("outside").context("create outside")?;
    let workspace_handle = Dir::open_ambient_dir(&workspace, ambient_authority())
        .context("open the workspace capability")?;
    workspace_handle
        .symlink("../outside", "escape")
        .context("create the escaping directory link")?;
    ensure!(
        outside.is_dir(),
        "the escape target must exist for the link to resolve"
    );

    let root = root_at(&workspace)?;
    let relative = root.resolve(Utf8Path::new("escape/secret"))?.to_path_buf();
    let err = root
        .open_parent(&relative)
        .expect_err("opening the escaping parent must fail");
    ensure!(
        is_capability_escape(&err),
        "the capability should report an escape, but reported {err}"
    );
    Ok(())
}
