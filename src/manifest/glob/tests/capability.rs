//! Tests for the capability handle the glob metadata checks run through.
use super::super::walk::literal_dir_prefix;
use super::super::walk::open_root_dir;
use super::super::{GlobPattern, glob_paths};
#[cfg(unix)]
use anyhow::{Context, Result, anyhow, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use minijinja::ErrorKind;
use rstest::{fixture, rstest};
use tempfile::{TempDir, tempdir};
use test_support::fs as test_fs;

/// A tree with one file inside `scoped/` and one sibling outside it.
///
/// Shared by the scoping tests so each asserts against the same layout:
/// `scoped/in.txt` is reachable through a capability rooted at `scoped/`,
/// whereas `out.txt` is not.
#[fixture]
fn scoped_tree() -> Result<TempDir> {
    let temp = tempdir()?;
    let scoped = temp.path().join("scoped");
    test_fs::create_dir(&scoped)?;
    test_fs::write(scoped.join("in.txt"), "in")?;
    test_fs::write(temp.path().join("out.txt"), "out")?;
    Ok(temp)
}

#[cfg(unix)]
#[rstest]
#[case("src/*.c", "src/")]
#[case("src/sub/**/*.c", "src/sub/")]
#[case("*.c", ".")]
#[case("a.txt", ".")]
#[case("/tmp/x/*.txt", "/tmp/x/")]
#[case("/*.txt", "/")]
#[case("src/a.txt", "src/")]
#[case("src/{a,b}/*.c", "src/")]
// A bracketed literal escape names a character, not a wildcard, so the scan
// steps over it and keeps the directories beyond it in the prefix.
#[case("src/[*]x/generated/*.c", "src/[*]x/generated/")]
#[case("[[]dir/*.c", "[[]dir/")]
// A genuine character class is a wildcard and still stops the scan.
#[case("src/[ab]x/*.c", "src/")]
#[case("src/[a]x/*.c", "src/")]
fn literal_dir_prefix_stops_at_first_metacharacter(#[case] pattern: &str, #[case] expected: &str) {
    assert_eq!(literal_dir_prefix(pattern), expected, "pattern {pattern}");
}

/// A prefix naming nothing, and one naming a regular file, must both yield no
/// capability at all — not merely an empty match set, which an unscoped
/// implementation would also produce.
#[rstest]
#[case("no-such-dir", "missing directory")]
#[case("out.txt", "regular file")]
fn open_root_dir_declines_unopenable_prefix(
    scoped_tree: Result<TempDir>,
    #[case] prefix: &str,
    #[case] desc: &str,
) -> Result<()> {
    let temp = scoped_tree?;
    let pattern = GlobPattern::new(&format!("{}/{prefix}/*.txt", temp.path().display()))?;
    let root = open_root_dir(pattern.normalized(), None)
        .with_context(|| format!("open root for {desc}"))?;
    ensure!(
        root.is_none(),
        "{desc} prefix should yield no capability at all"
    );
    ensure!(
        glob_paths(pattern.raw(), None)?.is_empty(),
        "{desc} prefix should expand to no matches"
    );
    Ok(())
}

/// A directory whose name contains a glob metacharacter is still a literal
/// directory, so the capability reaches into it rather than stopping at its
/// parent — and the prefix is unescaped before it meets the filesystem.
#[cfg(unix)]
#[test]
fn open_root_dir_scopes_past_an_escaped_metacharacter() -> Result<()> {
    let temp = tempdir()?;
    let odd = temp.path().join("*x");
    test_fs::create_dir(&odd)?;
    test_fs::write(odd.join("in.txt"), "in")?;
    test_fs::write(temp.path().join("out.txt"), "out")?;

    let pattern = GlobPattern::new(&format!(r"{}/\*x/*.txt", temp.path().display()))?;
    let root = open_root_dir(pattern.normalized(), None)
        .context("open capability root")?
        .ok_or_else(|| anyhow!("the escaped directory exists, so a root was expected"))?;

    let expected_prefix = Utf8PathBuf::try_from(odd)?;
    ensure!(
        root.prefix() == expected_prefix,
        "capability should reach the escaped directory, got {prefix}",
        prefix = root.prefix()
    );
    ensure!(
        root.dir().metadata("in.txt").is_ok(),
        "the escaped directory's contents must be reachable"
    );

    let results = glob_paths(pattern.raw(), None)?;
    ensure!(
        results.iter().all(|p| p.ends_with("in.txt")) && results.len() == 1,
        "expected only the file inside the escaped directory: {results:?}"
    );
    Ok(())
}

/// A literal prefix must not follow a symbolic link before its capability is
/// established, even when the linked directory would contain a valid match.
#[cfg(unix)]
#[test]
fn open_root_dir_rejects_a_symlinked_literal_prefix() -> Result<()> {
    let temp = tempdir()?;
    let target = temp.path().join("target");
    test_fs::create_dir(&target)?;
    test_fs::write(target.join("in.c"), "in")?;
    test_fs::symlink("target", temp.path().join("link"))?;

    let pattern = GlobPattern::new(&format!("{}/link/*.c", temp.path().display()))?;
    ensure!(
        open_root_dir(pattern.normalized(), None).is_err(),
        "a symbolic-link prefix must not receive a capability"
    );
    ensure!(
        glob_paths(pattern.raw(), None).is_err(),
        "a symbolic-link prefix must fail rather than traverse its target"
    );
    Ok(())
}

/// Restores a directory's mode so the temporary tree can still be removed.
#[cfg(unix)]
struct ModeGuard(Utf8PathBuf);

#[cfg(unix)]
impl Drop for ModeGuard {
    fn drop(&mut self) {
        if let Err(err) = test_fs::set_mode(&self.0, 0o755) {
            tracing::warn!("failed to restore mode on {path}: {err}", path = self.0);
        }
    }
}

/// A prefix that exists but cannot be opened is a genuine failure, not an
/// empty match set: only a missing or non-directory prefix short-circuits, so
/// anything else must reach the caller as an error.
#[cfg(unix)]
#[test]
fn open_root_dir_propagates_an_unreadable_prefix() -> Result<()> {
    let temp = tempdir()?;
    let locked = temp.path().join("locked");
    test_fs::create_dir(&locked)?;
    test_fs::create_dir(locked.join("inner"))?;
    test_fs::write(locked.join("inner").join("in.txt"), "in")?;
    test_fs::set_mode(&locked, 0o000)?;
    let _restore = ModeGuard(Utf8PathBuf::try_from(locked.clone())?);

    let pattern = GlobPattern::new(&format!("{}/inner/*.txt", locked.display()))?;
    let Err(err) = open_root_dir(pattern.normalized(), None) else {
        // A privileged user bypasses the mode, so there is nothing to observe.
        tracing::warn!("skipping: the mode-000 prefix stayed readable");
        return Ok(());
    };
    ensure!(
        err.kind() == std::io::ErrorKind::PermissionDenied,
        "unexpected error kind {kind:?}",
        kind = err.kind()
    );

    let expansion = glob_paths(pattern.raw(), None)
        .expect_err("an unreadable prefix must fail the expansion, not silently match nothing");
    ensure!(
        expansion.kind() == ErrorKind::InvalidOperation,
        "unexpected error kind {kind:?}",
        kind = expansion.kind()
    );
    Ok(())
}

#[rstest]
fn open_root_dir_scopes_capability_to_literal_prefix(scoped_tree: Result<TempDir>) -> Result<()> {
    // The capability must be opened at the pattern's literal prefix, not at
    // the filesystem root, so a sibling of the prefix is unreachable through
    // the handle even though it exists on disk.
    let temp = scoped_tree?;
    let pattern = GlobPattern::new(&format!("{}/scoped/*.txt", temp.path().display()))?;
    let root = open_root_dir(pattern.normalized(), None)
        .context("open capability root")?
        .ok_or_else(|| anyhow!("literal prefix exists, so a root was expected"))?;

    let expected_prefix = Utf8PathBuf::try_from(temp.path().join("scoped"))?;
    ensure!(
        root.prefix() == expected_prefix,
        "capability prefix {prefix} should be the literal prefix {expected_prefix}",
        prefix = root.prefix()
    );
    ensure!(
        root.dir().metadata("in.txt").is_ok(),
        "files under the prefix must be reachable through the capability"
    );
    ensure!(
        root.dir().metadata("../out.txt").is_err(),
        "the capability must not reach outside the literal prefix"
    );
    Ok(())
}

/// The walker yields matches rooted the way the pattern was, so they only
/// resolve through the scoped handle once relativised against the prefix.
#[rstest]
fn glob_root_relativises_matches_against_the_prefix(scoped_tree: Result<TempDir>) -> Result<()> {
    let temp = scoped_tree?;
    let root_path =
        Utf8PathBuf::try_from(temp.path().to_path_buf()).context("temp dir path is not UTF-8")?;
    let pattern = GlobPattern::new(&format!("{root_path}/scoped/*.txt"))?;
    let root = open_root_dir(pattern.normalized(), None)
        .context("open capability root")?
        .ok_or_else(|| anyhow!("literal prefix exists, so a root was expected"))?;

    let metadata = root
        .metadata(&root_path.join("scoped/in.txt"))
        .context("metadata for a match inside the prefix")?
        .ok_or_else(|| anyhow!("in.txt should resolve through the capability"))?;
    ensure!(metadata.is_file(), "in.txt should be a regular file");

    // A sibling outside the prefix cannot be relativised, so it is rejected
    // rather than silently resolved through a wider capability.
    let err = root
        .metadata(&root_path.join("out.txt"))
        .expect_err("a sibling outside the prefix must not resolve");
    ensure!(
        err.kind() == std::io::ErrorKind::InvalidInput,
        "unexpected error kind {kind:?}",
        kind = err.kind()
    );
    Ok(())
}

#[rstest]
fn glob_paths_matches_only_within_literal_prefix(scoped_tree: Result<TempDir>) -> Result<()> {
    let temp = scoped_tree?;
    let pattern = format!("{}/scoped/*.txt", temp.path().display());
    let results = glob_paths(&pattern, None)?;
    ensure!(
        results.iter().all(|p| p.ends_with("in.txt")),
        "only files under the literal prefix should match: {results:?}"
    );
    ensure!(results.len() == 1, "expected one match: {results:?}");
    Ok(())
}

#[test]
fn glob_paths_matches_parent_relative_patterns() -> Result<()> {
    // Injecting a subdirectory as the glob base reaches patterns that ascend
    // out of it: a `..` component in a match used to require changing the
    // process working directory, which the sandbox-escape rejection cannot
    // allow. The base's parent — here the temporary directory — is isolated,
    // so the result is deterministic.
    let temp = tempdir()?;
    let sub = temp.path().join("sub");
    test_fs::create_dir(&sub)?;
    test_fs::write(temp.path().join("out.txt"), "out")?;

    let results = glob_paths(
        "../*.txt",
        Some(Utf8Path::from_path(&sub).expect("temp paths are UTF-8")),
    )?;
    ensure!(
        results == vec!["../out.txt".to_owned()],
        "expected the parent-relative match, got {results:?}"
    );
    Ok(())
}

/// A symbolic link pointing out of the literal prefix is unreadable through
/// the capability. It names no file the expansion can offer, so it is skipped
/// rather than aborting the whole traversal — whether the link is the match's
/// final component or an intermediate directory it is reached through.
#[cfg(unix)]
#[rstest]
#[case::final_component("src/*.c", "escaped.c", "../vendor/escaped.c", "real.c")]
#[case::intermediate_directory("src/*/*.c", "link", "../vendor", "real/real.c")]
fn glob_paths_skips_symlinks_escaping_the_prefix(
    #[case] pattern_tail: &str,
    #[case] link_name: &str,
    #[case] link_target: &str,
    #[case] kept: &str,
) -> Result<()> {
    let temp = tempdir()?;
    let src = temp.path().join("src");
    let vendor = temp.path().join("vendor");
    test_fs::create_dir(&src)?;
    test_fs::create_dir(&vendor)?;
    test_fs::write(vendor.join("escaped.c"), "escaped")?;
    if let Some(parent) = Utf8Path::new(kept)
        .parent()
        .filter(|p| !p.as_str().is_empty())
    {
        test_fs::create_dir_all(src.join(parent))?;
    }
    test_fs::write(src.join(kept), "kept")?;
    test_fs::symlink(link_target, src.join(link_name))?;

    let pattern = format!("{}/{pattern_tail}", temp.path().display());
    let results =
        glob_paths(&pattern, None).context("an escaping symlink must not abort the walk")?;
    ensure!(
        results.iter().any(|p| p.ends_with(kept)),
        "valid matches should be preserved: {results:?}"
    );
    ensure!(
        results.iter().all(|p| !p.ends_with("escaped.c")),
        "a symlink resolving outside the prefix should be skipped: {results:?}"
    );
    Ok(())
}

/// A link with no target is absent rather than broken, so it is skipped like
/// an escaping one.
#[cfg(unix)]
#[test]
fn glob_paths_skips_dangling_symlinks() -> Result<()> {
    let temp = tempdir()?;
    let src = temp.path().join("src");
    test_fs::create_dir(&src)?;
    test_fs::write(src.join("real.c"), "real")?;
    test_fs::symlink("nowhere.c", src.join("dangling.c"))?;

    let pattern = format!("{}/src/*.c", temp.path().display());
    let results =
        glob_paths(&pattern, None).context("a dangling symlink must not abort the walk")?;
    ensure!(
        results.iter().any(|p| p.ends_with("real.c")),
        "valid matches should be preserved: {results:?}"
    );
    ensure!(
        results.iter().all(|p| !p.ends_with("dangling.c")),
        "a dangling symlink should be skipped: {results:?}"
    );
    Ok(())
}

/// A cyclic link describes a broken tree rather than an absent file, so it
/// must not be quietly dropped along with the escaping and dangling links.
#[cfg(unix)]
#[test]
fn glob_paths_reports_symlink_loops() -> Result<()> {
    let temp = tempdir()?;
    let src = temp.path().join("src");
    test_fs::create_dir(&src)?;
    test_fs::symlink("loop.c", src.join("loop.c"))?;

    let pattern = format!("{}/src/*.c", temp.path().display());
    let err = glob_paths(&pattern, None).expect_err("a symlink loop should surface as an error");
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "unexpected error kind {kind:?}",
        kind = err.kind()
    );
    Ok(())
}

#[test]
fn open_root_dir_falls_back_to_cwd_without_a_literal_prefix() -> Result<()> {
    // Patterns whose first component is a wildcard have no literal directory
    // component, so the capability stays scoped to the working directory —
    // the pre-existing behaviour for relative patterns.
    let pattern = GlobPattern::new("*.txt")?;
    let root = open_root_dir(pattern.normalized(), None)
        .context("open capability root")?
        .ok_or_else(|| anyhow!("the working directory always exists"))?;
    ensure!(
        root.prefix() == Utf8Path::new("."),
        "unexpected prefix {prefix}",
        prefix = root.prefix()
    );
    Ok(())
}
