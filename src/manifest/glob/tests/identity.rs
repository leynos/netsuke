//! Regression tests for the identity of Unix paths containing a literal
//! backslash.
//!
//! On Unix a backslash is an ordinary filename character, not a separator.
//! Result rebasing must therefore return the exact name that passed the
//! capability metadata check. Rewriting the backslash would report the match
//! under a different identity, and a name such as `part\..\other.txt` would
//! become the parent-directory traversal `part/../other.txt` that the checked
//! filesystem path never contained.
//!
//! Each case builds a disposable fixture and invokes the production
//! [`super::super::glob_paths`] boundary; neither the environment nor the
//! working directory of the test process is mutated.

#[cfg(unix)]
use super::super::glob_paths;
#[cfg(unix)]
use anyhow::{Context, Result, ensure};
#[cfg(unix)]
use camino::Utf8Path;
#[cfg(unix)]
use tempfile::tempdir;
#[cfg(unix)]
use test_support::fs as test_fs;

/// A literal backslash stays in the returned name rather than addressing a
/// different file that the capability check never saw.
#[cfg(unix)]
#[test]
fn glob_paths_preserves_a_literal_backslash_filename() -> Result<()> {
    let temp = tempdir()?;
    test_fs::create_dir(temp.path().join("nested"))?;
    test_fs::write(temp.path().join("nested").join("leaf.txt"), "decoy")?;
    test_fs::write(temp.path().join(r"nested\leaf.txt"), "attacker")?;
    let base = Utf8Path::from_path(temp.path()).context("temporary paths must be UTF-8")?;

    let results = glob_paths("*.txt", Some(base))?;

    ensure!(
        results == vec![r"nested\leaf.txt".to_owned()],
        "a literal backslash must be preserved, got {results:?}"
    );
    let returned = results
        .first()
        .context("the matched literal-backslash file must be returned")?;
    let contents = test_fs::read_to_string(base.join(returned))?;
    ensure!(
        contents == "attacker",
        "the returned path must name the matched file, not the {contents:?} decoy"
    );
    Ok(())
}

/// A literal `..` spelled between backslashes stays part of one filename
/// component; it must not become parent-directory traversal after rebasing.
#[cfg(unix)]
#[test]
fn glob_paths_does_not_introduce_parent_traversal() -> Result<()> {
    let temp = tempdir()?;
    test_fs::create_dir(temp.path().join("part"))?;
    test_fs::write(temp.path().join("other.txt"), "decoy")?;
    test_fs::write(temp.path().join(r"part\..\other.txt"), "attacker")?;
    let base = Utf8Path::from_path(temp.path()).context("temporary paths must be UTF-8")?;

    // Match only the literal-backslash name: the decoy that a rewritten
    // separator would reach is `other.txt`, and it must not be a legitimate
    // match of the pattern.
    let results = glob_paths("part*", Some(base))?;

    ensure!(
        results == vec![r"part\..\other.txt".to_owned()],
        "the literal backslash spelling must survive, got {results:?}"
    );
    let returned = results
        .first()
        .context("the matched literal-backslash file must be returned")?;
    ensure!(
        !returned.contains("/../"),
        "rebasing must not create a parent-directory component: {returned:?}"
    );
    let contents = test_fs::read_to_string(base.join(returned))?;
    ensure!(
        contents == "attacker",
        "the returned path must name the matched file, not the {contents:?} decoy"
    );
    Ok(())
}

/// An absolute pattern with no injected base also keeps the literal
/// backslash, so direct callers see the identity the walk selected.
#[cfg(unix)]
#[test]
fn glob_paths_preserves_a_literal_backslash_in_an_absolute_match() -> Result<()> {
    let temp = tempdir()?;
    test_fs::write(temp.path().join(r"nested\leaf.txt"), "attacker")?;
    let pattern = format!("{}/*.txt", temp.path().display());

    let results = glob_paths(&pattern, None)?;

    let expected = format!(r"{}/nested\leaf.txt", temp.path().display());
    ensure!(
        results == vec![expected],
        "the absolute match must keep its literal backslash, got {results:?}"
    );
    Ok(())
}
