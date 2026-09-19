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

use super::is_scanned;
use super::roots::{MACHINE_LOCAL_DIRECTORIES, collect_all_sources, collect_rust_sources};
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
/// different hat. An empty `core.excludesFile` covers both spellings a global
/// ignore can take: it overrides a configured path, and it also suppresses the
/// default `~/.config/git/ignore`, measured against both. One flag is enough:
/// an earlier version of this comment named a second key for the default path,
/// and that key does not exist in git.
///
/// A template directory is a fourth, and it is closed at `git init` above
/// rather than here, because the `info/exclude` it seeds is written before this
/// runs and no later call could unpin it.
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
    // `--template=` is what makes the scratch repository answer from the
    // `.gitignore` alone. A template directory can hold an `info/exclude`, and
    // git writes it into the new repository, where `check-ignore` reads it;
    // measured at a false pass once seeded. An empty value suppresses the
    // template, and it is the only form that does: `GIT_TEMPLATE_DIR` outranks
    // a `-c init.templateDir=` given to the same command, measured, so a
    // contributor with that variable set would otherwise see the false pass
    // survive.
    //
    // The flag belongs here rather than on `check-ignore` because
    // `info/exclude` is written at *init* time, so there is no later call that
    // could unpin it. That is the whole of its job, and it is worth stating
    // narrowly: a template can seed `.git/config` too, but the helper's own
    // `-c core.excludesFile=` already neutralizes an ignore file configured
    // there — measured with a template seeding only `.git/config`, unpinned and
    // pinned both leaving the name unignored. An earlier version of this
    // comment claimed the config half as well and was wrong about it.
    let init = std::process::Command::new("git")
        .args(["init", "--quiet", "--template="])
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

/// Fail if a cache inside a scanned root is read rather than skipped.
///
/// The scan descends a root list, and a scan is not a walk of the repository:
/// a root covers nested directory names too, so the roots shipped without any
/// skip rule and a `.uv-cache` under `tests/` would have been read. That is how
/// the scan and this walk came apart — the walk has always skipped by name, so
/// the cache was invisible to it and ungoverned by it, while the scan read it.
/// Measured before the fix: a vendored source carrying the banned `allow` under
/// `tests/.uv-cache/` failed the scan contract, which is red on a machine where
/// a tool had run and green on a fresh clone, with the offending path in no
/// diff and in no `git status` because the name is git-ignored. A verdict that
/// depends on a machine is worse than no verdict, so the two walks must agree.
///
/// The tree is synthetic for the usual reason: the disagreement needs a cache
/// inside a scanned root, and the repository has none today, which is the shape
/// that would otherwise go unnoticed until a contributor's tooling created one.
#[test]
fn a_cache_inside_a_scanned_root_is_skipped_by_the_scan() -> Result<()> {
    let scratch = tempdir().context("create a scratch directory for the walk")?;
    let scratch_path = Utf8Path::from_path(scratch.path())
        .context("a temporary directory path should be valid UTF-8")?;
    let root = Dir::open_ambient_dir(scratch_path, ambient_authority())
        .context("open the scratch directory")?;
    for directory in ["tests/.uv-cache", "tests/nested/target", "tests"] {
        root.create_dir_all(directory)
            .with_context(|| format!("create `{directory}`"))?;
    }
    root.write("tests/.uv-cache/vendored.rs", b"fn vendored() {}\n")
        .context("write the cached source")?;
    root.write("tests/nested/target/generated.rs", b"fn generated() {}\n")
        .context("write the nested generated source")?;
    root.write("tests/kept.rs", b"fn kept() {}\n")
        .context("write the kept source")?;

    let mut read = Vec::new();
    collect_rust_sources(&root, "tests", &mut read)?;
    let mut scanned: Vec<&str> = read.iter().map(|(path, _)| path.as_str()).collect();
    scanned.sort_unstable();

    let mut walked = Vec::new();
    collect_all_sources(&root, ".", &mut walked)?;
    walked.sort();

    // `collect_rust_sources` is the function the scan actually reads through, so
    // this is the assertion the missing skip failed. Asserting on
    // `collect_all_sources` alone would not have caught it: that walk has always
    // skipped by name, so it was the one behaving correctly.
    ensure!(
        scanned == ["tests/kept.rs"],
        "the scan must skip a machine-local name inside a scanned root, at any \
         depth, or its verdict depends on which tools have run on this machine; \
         got {scanned:?}"
    );
    // And the two walks agree, which is the property that keeps the coverage
    // invariant meaningful: it reports a source as ungoverned only when the scan
    // really would not read it.
    ensure!(
        walked == scanned,
        "the scan and the workspace walk must agree on what is governed, or the \
         coverage invariant excuses a source the scan reads or reports one it \
         does not; scan {scanned:?}, walk {walked:?}"
    );
    Ok(())
}

/// Return whether the repository at `root` ignores a source under `name`.
///
/// The machine's global ignore file is disabled first, so the answer comes from
/// the repository copied into `root` and from nothing else. `core.excludesFile`
/// is the only key that needs setting: an empty value overrides a configured
/// path and equally suppresses the default `~/.config/git/ignore`, so one flag
/// covers both ways a contributor's machine can answer for the repository.
/// Measured against both configurations. The value is empty rather than
/// `/dev/null` because a device path is a Unix spelling and this test runs on
/// the Windows lane too; the empty form needs no filesystem path and was
/// measured to behave identically.
///
/// `check-ignore -q` reports by exit status: 0 ignored, 1 not ignored. Every
/// other status is a real failure and propagates, so "git could not answer" is
/// never read as "git would track this".
fn is_ignored(root: &Utf8Path, name: &str) -> Result<bool> {
    let status = std::process::Command::new("git")
        .args([
            "-c",
            "core.excludesFile=",
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
