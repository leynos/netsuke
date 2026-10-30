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

use super::{ReadingRoot, is_capability_escape};

/// A reading root bound to `directory` with its absolute spelling recorded.
fn root_at(directory: &Utf8Path) -> Result<ReadingRoot> {
    root_recorded_at(directory, directory)
}

/// A reading root bound to `directory` but recorded under `recorded`.
///
/// Lexical resolution never opens through the recorded spelling, so a case can
/// name a stable root such as `/tmp/work` without creating it.
fn root_recorded_at(directory: &Utf8Path, recorded: &Utf8Path) -> Result<ReadingRoot> {
    let handle = Dir::open_ambient_dir(directory, ambient_authority())
        .with_context(|| format!("open reading root {directory}"))?;
    Ok(ReadingRoot::new(
        Arc::new(handle),
        Some(recorded.to_path_buf()),
    ))
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
#[case::inside_parent("sub/../file", "file")]
#[case::redundant_separators("sub//inner.txt", "sub/inner.txt")]
fn relative_requests_normalise_beneath_the_root(
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
#[case::parent_above_root("sub/../../outside/secret")]
#[case::only_parent("..")]
fn relative_requests_escaping_the_root_are_refused(#[case] requested: &str) -> Result<()> {
    let (_temp, _path, root) = scratch()?;
    let err = root
        .resolve(Utf8Path::new(requested))
        .expect_err("an escaping relative request must be refused");
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "{requested:?}: should report InvalidOperation but was {:?}",
        err.kind()
    );
    Ok(())
}

#[rstest]
fn absolute_requests_beneath_the_recorded_root_are_accepted() -> Result<()> {
    let (_temp, path, _) = scratch()?;
    let root = root_recorded_at(&path, Utf8Path::new("/tmp/work"))?;
    ensure!(
        resolved(&root, "/tmp/work/file").as_deref() == Some(Utf8Path::new("file")),
        "an absolute path beneath the root should resolve relatively"
    );
    ensure!(
        resolved(&root, "/tmp/work/sub/../file").as_deref() == Some(Utf8Path::new("file")),
        "an absolute path should be normalized after the root is stripped"
    );
    Ok(())
}

#[rstest]
#[case::outside("/tmp/other/secret")]
#[case::sibling_prefix("/tmp/workspace/file")]
#[case::escapes_root("/tmp/work/../other/secret")]
fn absolute_requests_outside_the_recorded_root_are_refused(#[case] requested: &str) -> Result<()> {
    let (_temp, _path, root) = scratch()?;
    ensure!(
        resolved(&root, requested).is_none(),
        "{requested:?} must be refused as outside the workspace"
    );
    Ok(())
}

/// Without an absolute spelling there is no safe way to recognize an absolute
/// request, so it is refused rather than guessed at.
#[rstest]
fn absolute_requests_need_a_recorded_root_path() -> Result<()> {
    let handle = Dir::open_ambient_dir(".", ambient_authority())
        .context("open the working directory as a capability")?;
    let root = ReadingRoot::new(Arc::new(handle), None);
    ensure!(
        resolved(&root, "/tmp/work/file").is_none(),
        "an absolute request without a recorded root path must be refused"
    );
    Ok(())
}

/// The refusal names the request and the boundary, never the file's contents.
#[rstest]
fn the_refusal_is_a_localized_workspace_diagnostic() -> Result<()> {
    let (_temp, _path, root) = scratch()?;
    let err = root
        .resolve(Utf8Path::new("/tmp/other/secret"))
        .expect_err("an outside path must be refused");
    let message = err.to_string();
    ensure!(
        message.contains("outside the workspace") || message.contains("workspace"),
        "the diagnostic should name the workspace boundary: {message}"
    );
    ensure!(
        !message.contains("TOP-SECRET"),
        "the diagnostic must not disclose file contents: {message}"
    );
    Ok(())
}

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

    let denied = std::io::Error::from_raw_os_error(13);
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
