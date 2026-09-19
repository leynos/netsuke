//! Self-tests for the workspace walk behind the coverage invariant.
//!
//! The invariant above it asserts that every Rust source in the workspace is
//! scanned, and it can only assert that about the sources its walk reaches. So
//! the walk's own reach is pinned here, against a synthetic tree rather than
//! against the repository: a shape the repository does not happen to contain
//! today is exactly the shape that would go unnoticed if the test read the real
//! tree.
//!
//! The same reasoning governs the skip list's own justification, which is
//! checked against a scratch repository holding the repository's `.gitignore`
//! rather than against the working tree. A working tree answers with more than
//! the repository's rules — nested tools write ignore files of their own — so
//! asking it would make the answer depend on which tools had run.

use super::{MACHINE_LOCAL_DIRECTORIES, collect_all_sources, is_scanned};
use anyhow::{Context, Result, bail, ensure};
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

/// Fail if a skipped name is ignored only by a cache's own ignore file.
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
/// The question is asked of the *repository's* rules rather than of the working
/// tree, because a working tree answers with more than those: `git check-ignore`
/// also reads ignore files that nested tools write. Ruff drops a `.gitignore`
/// holding `*` into `.ruff_cache` as a side effect of running, so asking the
/// live tree made `.ruff_cache` pass on a machine where ruff had run and fail on
/// a fresh clone — and `make test` can precede `make lint`, so the answer would
/// have depended on the gate order. The rule has to hold on every checkout, so
/// the repository's `.gitignore` is copied into a scratch repository and the
/// question is put there. Nothing under the workspace is touched, and the answer
/// no longer depends on which tools have run or on whether the sources are a
/// checkout at all, which is also why no `git rev-parse` guard is needed for the
/// copies cargo-mutants makes: this test brings its own repository.
///
/// The machine's own git configuration is a third source of answers and is
/// switched off for the same reason. A contributor with a global ignore file
/// listing a name here would otherwise see the test pass while the repository
/// says nothing about that name, which is the original defect wearing a
/// different hat. Setting `core.excludesFile` to `/dev/null` covers both
/// spellings a global ignore can take: it overrides a configured path, and it
/// also suppresses the default `~/.config/git/ignore`, measured with the
/// default present and no path configured — a bare path-setting flag is not
/// needed for the second, and had been written here as though it were.
///
/// `.git` is the one legitimate exception: git refuses to track anything
/// beneath it whatever the ignore files say, so the appeal still holds even
/// though `check-ignore` reports it as unignored. It is named here rather than
/// excluded by a pattern, so a future name added to the list without an ignore
/// rule is caught rather than grandfathered in.
#[test]
fn every_skipped_name_is_one_git_would_not_track() -> Result<()> {
    let root = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
        .context("open the workspace root")?;
    let scratch = tempdir().context("create a scratch repository")?;
    let scratch_path = Utf8Path::from_path(scratch.path())
        .context("a temporary directory path should be valid UTF-8")?;
    let scratch_root = Dir::open_ambient_dir(scratch_path, ambient_authority())
        .context("open the scratch directory")?;
    let ignore_rules = root
        .read(".gitignore")
        .context("read the repository's `.gitignore`")?;
    scratch_root
        .write(".gitignore", ignore_rules)
        .context("copy the repository's `.gitignore` into the scratch repository")?;
    let init = std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(scratch_path)
        .status()
        .context("run git init in the scratch repository")?;
    ensure!(init.success(), "git init failed in the scratch repository");

    let mut unexplained = Vec::new();
    for name in MACHINE_LOCAL_DIRECTORIES {
        if name == ".git" {
            continue;
        }
        if !is_ignored(scratch_path, name)? {
            unexplained.push(name);
        }
    }
    ensure!(
        unexplained.is_empty(),
        "these names are skipped by the walk, but the repository's own `.gitignore` does \
         not ignore them, so git would track a source under them and the walk would hide \
         it rather than report it; add each to `.gitignore` beside its sibling caches, or \
         reconsider the skip: {unexplained:?}"
    );
    Ok(())
}

/// Return whether the repository at `root` ignores a source under `name`.
///
/// The machine's global ignore file is disabled first, so the answer comes from
/// the repository copied into `root` and from nothing else. `core.excludesFile`
/// is the only key that needs setting: `/dev/null` overrides a configured path
/// and equally suppresses the default `~/.config/git/ignore`, so one flag covers
/// both ways a contributor's machine can answer for the repository.
///
/// `check-ignore -q` reports by exit status: 0 ignored, 1 not ignored. Every
/// other status is a real failure and propagates, so "git could not answer" is
/// never read as "git would track this".
fn is_ignored(root: &Utf8Path, name: &str) -> Result<bool> {
    let status = std::process::Command::new("git")
        .args([
            "-c",
            "core.excludesFile=/dev/null",
            "check-ignore",
            "-q",
            &format!("{name}/probe.rs"),
        ])
        .current_dir(root)
        .status()
        .context("run git check-ignore")?;
    match status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => bail!("git check-ignore failed ({status})"),
    }
}
