//! Tests for the injected base directory that anchors relative glob patterns.
//!
//! [`super::glob_paths`] accepts an optional base: relative patterns are joined
//! onto it before matching and the base is stripped from the results. These
//! tests cover the two invariants of that anchoring — the base is not applied
//! twice, and a symlinked base is followed rather than rejected.
use super::super::glob_paths;
#[cfg(unix)]
use anyhow::{Context, Result, ensure};
use tempfile::{Builder, tempdir};
use test_support::fs as test_fs;

/// A relative injected base is not reopened under itself.
///
/// `expand_glob` joins the base onto the pattern to build the search text
/// (`base.join(pattern)`); the capability root must then be opened from that
/// combined path rather than from the base a second time. Passing the base to
/// `open_root_dir` again would open `base` and then traverse the `base`
/// component once more, doubling the path and failing to match anything.
#[cfg(unix)]
#[test]
fn glob_paths_relative_base_is_not_doubled() -> Result<()> {
    let cwd = std::env::current_dir().context("read the process working directory")?;
    let temp = Builder::new()
        .prefix("f1-relative-base-")
        .tempdir_in(&cwd)
        .context("create a temporary directory under the working directory")?;
    let base = temp
        .path()
        .strip_prefix(&cwd)
        .context("the temporary directory must live under the working directory")?;
    test_fs::write(temp.path().join("a.txt"), "a")?;

    let results = glob_paths("*.txt", Some(base))?;
    ensure!(
        results == vec!["a.txt".to_owned()],
        "a relative base must not be doubled, got {results:?}"
    );
    Ok(())
}

/// A symlinked base is followed, but a symlink inside the pattern's own
/// literal prefix is still rejected.
///
/// The injected base is the workspace root the manifest was opened through,
/// which the shell may reach via a symbolic link. Anchoring the capability at
/// that base must follow the link; the pattern's literal prefix components are
/// still walked without following symlinks, so `glob_paths` with a symlinked
/// base expands matches rather than rejecting the whole glob.
#[cfg(unix)]
#[test]
fn glob_paths_follows_a_symlinked_base() -> Result<()> {
    let temp = tempdir()?;
    let target = temp.path().join("target");
    test_fs::create_dir(&target)?;
    test_fs::write(target.join("a.txt"), "a")?;
    let link = temp.path().join("link");
    test_fs::symlink("target", &link)?;

    let results = glob_paths("*.txt", Some(&link))?;
    ensure!(
        results == vec!["a.txt".to_owned()],
        "expected the match relative to the symlinked base, got {results:?}"
    );
    Ok(())
}
