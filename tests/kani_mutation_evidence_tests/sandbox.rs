//! An isolated copy of the tracked tree for the compile gate to mutate.
//!
//! The compile gate seeds each mutation patch into the tree it compiles, and
//! the checkout a developer is working in is the wrong place to do that.
//! Nextest terminates a timed-out test by signalling its process group, so
//! `Drop` cannot run and a seeded mutation can survive the run in the working
//! tree. Seeding into a copy removes the need for that cleanup to be
//! reliable: an abandoned copy is regenerated, whereas an abandoned mutation
//! in a live checkout is a change nobody made.
//!
//! # What a sandbox is
//!
//! [`Sandbox::create`] exports a revision of the tree with `git archive` and
//! extracts it under `target/`, which is ignored. The result carries no
//! `.git`, so no `git` command run inside it can reach the real repository.
//! [`Sandbox::isolation_env`] pins that rather than relying on it: `git`
//! walks parent directories looking for a `.git`, and the sandbox sits inside
//! the checkout, so without a ceiling the walk would find the real repository
//! and apply the patch to the live tree.
//!
//! # Which revision, and why not simply `HEAD`
//!
//! The revision is the working tree as the developer has it, captured by
//! `git stash create` — not `HEAD`. It is `HEAD` only when the tree is clean.
//!
//! `HEAD` alone would reintroduce this gate's own failure mode one level up.
//! A developer editing a patch and running `make test-kani-mutations` to check
//! it before committing would have the gate compile the *committed* patch and
//! report it healthy, which is a green run describing a tree nobody is
//! looking at. Capturing the working tree means the gate compiles what the
//! developer actually has; on CI the tree is clean, so the capture is empty
//! and the revision is `HEAD` exactly as before.
//!
//! `git stash create` is the right primitive for this rather than `git stash
//! push`. It writes a commit and *does not touch the working tree or the
//! stash stack*: measured at zero entries before and after, which matters on
//! this repository because the stack is shared across worktrees and a bare
//! `stash` can pop another session's work. It also captures only tracked
//! content, so a patch the developer has not yet `git add`ed is invisible to
//! the gate — the same way it is invisible to `git archive`.
//!
//! # Why `git archive` rather than a copy of the checkout
//!
//! Both the archive and a copy could hold the captured revision, so the choice
//! is about cost and completeness. An archive costs one `git` invocation plus
//! one `tar` extraction — measured at 0.1 s and 16 MB for this repository —
//! against the 264 MB a plain copy would walk, and it contains exactly the
//! tracked revision rather than dragging in `target/` and any untracked
//! scratch. A copy would also have to decide what to do with the `.git` it
//! carries, which the archive simply does not have.
//!
//! Extraction reproduces the archive faithfully, including the one tracked
//! symlink (`CRUSH.md`), whose mode `tar` preserves. No path component of the
//! archive escapes the extraction directory: `git archive` emits no `..` or
//! absolute names.
//!
//! # The cost, and why the path is fixed
//!
//! A build tree is keyed to the absolute paths it was compiled from, so a
//! sandbox at a fresh temporary path would invalidate the shared target
//! directory on every run and pay a cold build each time. The sandbox
//! therefore lives at a fixed path under `target/` and is emptied and
//! re-populated, which keeps the target directory valid across runs while
//! still compiling this run's tree. Measured on a warm tree: 36.3 s for the
//! first codegen, and 5.1 s for a re-extracted sandbox at the same path.

use std::process::Command;

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

/// Directory name the sandbox is created as, relative to `target/`.
///
/// Inside `target/` because that directory is ignored: a sandbox must never
/// appear in `git status`, and a crash that leaves one behind must not
/// require a cleanup step before the next run can start. The name is the only
/// component the sandbox removal has to name, so the parent handle stays the
/// ambient `target/` directory.
const SANDBOX_NAME: &str = "kani-mutation-sandbox";

/// A revision of the tracked tree, extracted into a fresh isolated directory.
///
/// Dropping a sandbox does not remove it. The next [`Sandbox::create`]
/// empties and re-populates the directory in place, so a leftover sandbox is
/// reclaimed rather than accumulated, and its cost to the next run is
/// nothing. Leaving it also keeps the shared target directory valid, which
/// deleting the tree would not.
pub(super) struct Sandbox {
    /// Absolute path of the extracted tree.
    ///
    /// This is the directory each patched build runs in, and the one the
    /// mutation patches are applied to. It is absolute because it is handed
    /// to `git` and to `cargo` as a working directory, and because
    /// `GIT_CEILING_DIRECTORIES` resolves relative to the caller's working
    /// directory rather than to the tree it is meant to bound.
    tree: Utf8PathBuf,
    /// Absolute path of the directory the sandbox lives under.
    ///
    /// Held so the ceiling can name a path the upward search for a `.git`
    /// cannot pass: it is the sandbox's own root, one level above the tree.
    root: Utf8PathBuf,
}

impl Sandbox {
    /// Export the current revision into a fresh sandbox and return its tree.
    ///
    /// Any previous sandbox at the same path is removed first, so the tree
    /// compiled is this run's rather than one a previous run left patched.
    pub(super) fn create(manifest_dir: &Utf8Path) -> Result<Self> {
        // Both this path and `remove_sandbox` are built from `SANDBOX_NAME`,
        // so the directory created and the one emptied cannot drift apart.
        let root = manifest_dir.join("target").join(SANDBOX_NAME);
        let tree = root.join("tree");

        let target = Dir::open_ambient_dir(manifest_dir.join("target"), ambient_authority())
            .with_context(|| format!("open {}", manifest_dir.join("target")))?;
        remove_sandbox(&target)?;
        Dir::create_ambient_dir_all(&tree, ambient_authority())
            .with_context(|| format!("create {tree}"))?;

        let archive = root.join("revision.tar");
        export_revision(manifest_dir, &archive)?;
        extract(&archive, &tree)?;
        Ok(Self { tree, root })
    }

    /// The tree the gate applies patches to and compiles.
    pub(super) fn tree(&self) -> &Utf8Path {
        &self.tree
    }

    /// Environment that keeps `git` operations inside the sandbox.
    ///
    /// `GIT_CEILING_DIRECTORIES` stops the upward search for a `.git` at the
    /// sandbox root, so a `git apply` run in the tree reports "not a git
    /// repository" instead of finding the real checkout above it. Measured
    /// both ways: with the ceiling the probe exits 128 and an apply still
    /// succeeds, without it the probe exits 0.
    pub(super) fn isolation_env(&self) -> [(&'static str, String); 1] {
        [("GIT_CEILING_DIRECTORIES", self.root.clone().into_string())]
    }
}

/// Remove any previous sandbox, tolerating its absence.
///
/// Suspends the caller's capability boundary for the one removal, which is
/// why it takes the handle rather than a path: the handle already names the
/// directory the removal is confined to.
fn remove_sandbox(target: &Dir) -> Result<()> {
    match target.remove_dir_all(SANDBOX_NAME) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("remove target/{SANDBOX_NAME}")),
    }
}

/// Write the revision the gate should compile to `archive` as a tar file.
///
/// `--format=tar` rather than the default, so the extraction step is one
/// command with no decompressor to locate.
fn export_revision(manifest_dir: &Utf8Path, archive: &Utf8Path) -> Result<()> {
    let revision = capture_revision(manifest_dir)?;
    let output = Command::new("git")
        .args([
            "archive",
            "--format=tar",
            &format!("--output={archive}"),
            &revision,
        ])
        .current_dir(manifest_dir)
        .output()
        .context("run git archive to export the tracked tree")?;
    ensure!(
        output.status.success(),
        "git archive failed: {}",
        String::from_utf8_lossy(&output.stderr).trim(),
    );
    Ok(())
}

/// The revision to compile: the working tree if it differs, else `HEAD`.
///
/// `git stash create` writes a commit holding the tracked working tree and
/// returns its id, or prints nothing when the tree is clean — in which case
/// `HEAD` is what the working tree is. It leaves the tree and the stash stack
/// alone, so this is a read even though it creates a commit.
///
/// The gate compiles this rather than `HEAD` so that a developer who edits a
/// patch and runs the gate before committing is checking the edit. On CI the
/// tree is clean and this resolves to `HEAD`.
fn capture_revision(manifest_dir: &Utf8Path) -> Result<String> {
    let output = Command::new("git")
        .args(["stash", "create"])
        .current_dir(manifest_dir)
        .output()
        .context("run git stash create to capture the working tree")?;
    ensure!(
        output.status.success(),
        "git stash create failed: {}",
        String::from_utf8_lossy(&output.stderr).trim(),
    );
    let captured = String::from_utf8(output.stdout)
        .context("decode captured revision id")?
        .trim()
        .to_owned();
    if captured.is_empty() {
        return Ok("HEAD".to_owned());
    }
    Ok(captured)
}

/// Extract `archive` into `tree`.
fn extract(archive: &Utf8Path, tree: &Utf8Path) -> Result<()> {
    let output = Command::new("tar")
        .args([
            "--extract",
            "--file",
            archive.as_str(),
            "--directory",
            tree.as_str(),
        ])
        .output()
        .context("run tar to extract the exported tree")?;
    ensure!(
        output.status.success(),
        "tar failed to extract the exported tree: {}",
        String::from_utf8_lossy(&output.stderr).trim(),
    );
    Ok(())
}
