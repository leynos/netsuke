//! Self-tests for the workspace walk behind the coverage invariant.
//!
//! The invariant above it asserts that every Rust source in the workspace is
//! scanned, and it can only assert that about the sources its walk reaches. So
//! the walk's own reach is pinned here, against a synthetic tree rather than
//! against the repository: a shape the repository does not happen to contain
//! today is exactly the shape that would go unnoticed if the test read the real
//! tree.

use super::{MACHINE_LOCAL_DIRECTORIES, collect_all_sources, is_scanned};
use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};
use tempfile::tempdir;

/// Fail if an `.rs` file under a non-cache dot-directory is walked past.
///
/// This is the shape the machine-local list must not swallow. A dot-prefix is
/// not evidence of a cache — `.config`, `.github`, and `.rules` are tracked
/// content, and a manifest can declare a target under any directory, hidden or
/// not — so a walk that skipped every dot-prefixed name would neither scan a
/// target in one nor report it, which is exactly the silent non-coverage the
/// invariant exists to prevent. The tree is synthetic so the assertion does not
/// depend on what this repository happens to contain today: a cache directory
/// and `target` still hold nothing, and a hidden directory holds a source that
/// must be named.
#[test]
fn a_source_under_a_dot_directory_is_walked_and_reported() -> Result<()> {
    let scratch = tempdir().context("create a scratch directory for the walk")?;
    let scratch_path = Utf8Path::from_path(scratch.path())
        .context("a temporary directory path should be valid UTF-8")?;
    let root = Dir::open_ambient_dir(scratch_path, ambient_authority())
        .context("open the scratch directory")?;
    for directory in [".hidden", "src", ".uv-cache", "target"] {
        root.create_dir_all(directory)
            .with_context(|| format!("create `{directory}`"))?;
    }
    root.write(".hidden/probe.rs", b"fn probe() {}\n")
        .context("write the hidden probe")?;
    root.write("src/kept.rs", b"fn kept() {}\n")
        .context("write the kept source")?;
    root.write(".uv-cache/vendored.rs", b"fn vendored() {}\n")
        .context("write the cached source")?;
    root.write("target/generated.rs", b"fn generated() {}\n")
        .context("write the generated source")?;

    let mut found = Vec::new();
    collect_all_sources(&root, ".", &mut found)?;
    found.sort();

    ensure!(
        found == [".hidden/probe.rs", "src/kept.rs"],
        "the walk should descend a non-cache dot-directory and skip the machine-local ones, \
         got {found:?}"
    );
    // The walk finding it is not enough; the invariant must also name it, which
    // is what fails the gate rather than silently excusing the source.
    ensure!(
        !is_scanned(".hidden/probe.rs"),
        "a source under a dot-directory is outside the scanned roots, so the invariant \
         must report it"
    );
    Ok(())
}

/// Fail if a skipped name is only skipped at the workspace root.
///
/// The skip is keyed on the entry name rather than on a workspace-relative
/// path, because that is the rule `.gitignore` states: its patterns carry no
/// leading slash, so `target/` and `memories/` are ignored at every depth. The
/// distinction is not cosmetic — a walk that skipped `target` only at the root
/// would descend a nested one, and if a nested `target` ever held a generated
/// `.rs` file the gate would turn on the compiler's output, which is the thing
/// it must not do. The tree is synthetic so the assertion does not depend on
/// the repository happening to have no nested cache today.
#[test]
fn a_machine_local_name_is_skipped_at_any_depth() -> Result<()> {
    let scratch = tempdir().context("create a scratch directory for the walk")?;
    let scratch_path = Utf8Path::from_path(scratch.path())
        .context("a temporary directory path should be valid UTF-8")?;
    let root = Dir::open_ambient_dir(scratch_path, ambient_authority())
        .context("open the scratch directory")?;
    for directory in ["tools/memories", "vendor/target", "src"] {
        root.create_dir_all(directory)
            .with_context(|| format!("create `{directory}`"))?;
    }
    root.write("tools/memories/probe.rs", b"fn probe() {}\n")
        .context("write the nested memories source")?;
    root.write("vendor/target/generated.rs", b"fn generated() {}\n")
        .context("write the nested target source")?;
    root.write("src/kept.rs", b"fn kept() {}\n")
        .context("write the kept source")?;

    let mut found = Vec::new();
    collect_all_sources(&root, ".", &mut found)?;
    found.sort();

    ensure!(
        found == ["src/kept.rs"],
        "the skip is by entry name, so a nested machine-local directory is skipped too; \
         got {found:?}"
    );
    Ok(())
}

/// Fail if the walk skips a name git would happily track.
///
/// The skip is justified by an appeal to `.gitignore`: a name git will not
/// track is not one a compiled source can live under, so skipping it cannot
/// hide anything. That appeal is only sound while it is *true* for every name in
/// the list, and it was not — `.netsuke` was skipped while `git check-ignore`
/// declined it, so a `.rs` file placed there would have been tracked, compiled,
/// skipped by the walk, and reported by nobody. That is the precise silent
/// non-coverage this invariant exists to prevent, and it arrived through the
/// list rather than through the walk.
///
/// `.git` is the one legitimate exception: git refuses to track anything
/// beneath it whatever the ignore files say, so the appeal still holds even
/// though `check-ignore` reports it as unignored. It is named here rather than
/// excluded by a pattern, so a future name added to the list without an ignore
/// rule is caught rather than grandfathered in.
#[test]
fn every_skipped_name_is_one_git_would_not_track() {
    let root = std::env::var("CARGO_MANIFEST_DIR").expect("the manifest directory should be set");
    let not_tracked = |name: &str| {
        let status = std::process::Command::new("git")
            .args(["check-ignore", "-q", &format!("{name}/probe.rs")])
            .current_dir(&root)
            .status()
            .expect("git check-ignore should run");
        status.success()
    };
    let unexplained: Vec<&str> = MACHINE_LOCAL_DIRECTORIES
        .iter()
        .copied()
        .filter(|name| *name != ".git" && !not_tracked(name))
        .collect();
    assert!(
        unexplained.is_empty(),
        "these names are skipped by the walk but git would track a source under them, \
         so the walk would hide it rather than report it; add each to `.gitignore` (its \
         sibling caches are already there) or reconsider the skip: {unexplained:?}"
    );
}
