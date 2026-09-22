//! Compile gate over every mutation patch's patched tree.
//!
//! `every_patch_applies_cleanly` in the parent module proves each patch
//! *applies*; it does not prove the patched tree *compiles* under the
//! configuration the harnesses are verified in. `make kani-full` denies
//! warnings (PR #714), so a patch that seeds its fault by leaving a binding or
//! helper unused is a hard compile error: `cargo kani` never reaches the
//! harness and the patch contributes no mutation evidence while still looking
//! healthy to `git apply --check`.
//!
//! This module closes that gap by applying each patch, compiling the patched
//! tree under `-D warnings`, and reverting. It is expensive — one Kani codegen
//! per patch — so the test is `#[ignore]`-gated and runs from its own Make
//! target on a shared `CARGO_TARGET_DIR`, which keeps successive patches
//! incremental.
//!
//! # Why Kani compiles the tree, not `cargo check`
//!
//! `cargo check` leaves `#[cfg(kani)]` code unparsed. A patch that seeds its
//! fault inside a Kani-gated item is therefore invisible to it: measured on
//! `ir__cycle__verification__self_dependency_reports_cycle`, whose only
//! changed line is a `cfg(kani)` match arm, `cargo check --lib --all-features`
//! exits 0 and reports the tree healthy while `cargo kani` rejects that same
//! tree with `variant Present is never constructed`. That is precisely the
//! failure this module exists to catch, so the gate compiles the tree the way
//! the harnesses are verified — through the Kani frontend.
//!
//! `--only-codegen` is a full compile under `cfg(kani)` with verification
//! skipped, so it subsumes the check it replaces rather than joining it: it
//! catches both the dead-code faults that motivated this gate and the
//! `cfg(kani)`-gated ones `cargo check` cannot see. Both were measured, in
//! both directions, before the swap.

use std::process::Command;

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

use super::{MUTATIONS_DIR, is_git_work_tree, manifest_dir};

/// Target directory shared by every patched build in one run.
///
/// Reused across patches on purpose: each patch touches one file, so only that
/// file recompiles and the run costs roughly one cold build plus a handful of
/// incremental ones. A per-patch directory would pay the cold cost each time.
const SHARED_TARGET_DIR: &str = "target/kani-mutation-compile";

/// The warning policy each patched tree is compiled under.
///
/// Restated rather than inherited from the ambient `RUSTFLAGS`: the gate this
/// mirrors is what `make kani-full` composes (`KANI_RUSTFLAGS`), and a test
/// that inherited instead would silently stop checking anything the day the
/// caller's environment changed.
const DENY_WARNINGS: &str = "-D warnings";

/// Apply `patch`, then revert it — explicitly on the normal path, and through
/// `Drop` when unwinding.
///
/// Both paths matter and they are not interchangeable. The explicit
/// [`Self::revert`] makes a failed reverse a test failure, so a run cannot
/// report success with a seeded mutation left in the working tree; `Drop`
/// covers the panic — or early return from a failed assertion — that would
/// otherwise strand a mutation for every later test to trip over.
struct AppliedPatch<'a> {
    /// Repository-relative path of the patch that was applied.
    patch: &'a Utf8Path,
    /// True while the patch is still in the working tree.
    ///
    /// Cleared by [`Self::revert`] so the `Drop` fallback does not attempt a
    /// second reverse, which would fail and log a spurious error on every
    /// successful patch.
    applied: bool,
}

impl AppliedPatch<'_> {
    /// Apply `patch` to the repository, failing when it does not apply.
    fn apply(patch: &Utf8Path) -> Result<AppliedPatch<'_>> {
        run_git_apply(["apply", patch.as_str()], patch, "apply")?;
        Ok(AppliedPatch {
            patch,
            applied: true,
        })
    }

    /// Revert the patch, propagating a failure to the caller.
    ///
    /// Reverting here rather than relying on `Drop` is what stops a failed
    /// reverse from passing quietly: without it the test returns `Ok(())` and
    /// the working tree keeps a mutation nothing reports.
    fn revert(mut self) -> Result<()> {
        let outcome = run_git_apply(
            ["apply", "--reverse", self.patch.as_str()],
            self.patch,
            "reverse",
        );
        self.applied = false;
        outcome
    }
}

impl Drop for AppliedPatch<'_> {
    fn drop(&mut self) {
        if !self.applied {
            return;
        }
        if let Err(err) = run_git_apply(
            ["apply", "--reverse", self.patch.as_str()],
            self.patch,
            "reverse",
        ) {
            // A panic is already unwinding when this runs, so the failure is
            // reported rather than raised: replacing the panic's message with
            // a revert error would hide the defect that caused it.
            tracing::error!("failed to revert {patch}: {err}", patch = self.patch);
        }
    }
}

/// Run one `git apply`-family invocation in the repository root.
fn run_git_apply<const N: usize>(args: [&str; N], patch: &Utf8Path, action: &str) -> Result<()> {
    let output = Command::new("git")
        .args(args)
        .current_dir(manifest_dir())
        .output()
        .with_context(|| format!("run git apply to {action} {patch}"))?;
    ensure!(
        output.status.success(),
        "git apply to {action} {patch} failed: {}",
        String::from_utf8_lossy(&output.stderr).trim(),
    );
    Ok(())
}

/// Compile the patched tree under `-D warnings`, returning its failure output.
///
/// `Ok(None)` means the tree compiled; `Ok(Some(stderr))` means it did not.
///
/// Invoked through the Kani frontend rather than `cargo check`, for the reason
/// the module docs give: only Kani parses `#[cfg(kani)]` code, so only Kani can
/// reject a patch that seeds its fault inside a Kani-gated item. `--only-codegen`
/// compiles without verifying, which is all this gate needs — the harnesses
/// themselves are run by `make kani-full`, not here.
fn compile_patched_tree(target_dir: &Utf8Path) -> Result<Option<String>> {
    let output = Command::new(env!("CARGO"))
        .args(["kani", "--only-codegen", "--lib", "--all-features"])
        .current_dir(manifest_dir())
        .env("RUSTFLAGS", DENY_WARNINGS)
        .env("CARGO_TARGET_DIR", target_dir)
        .output()
        .context("run cargo kani over the patched tree")?;
    if output.status.success() {
        return Ok(None);
    }
    Ok(Some(
        String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    ))
}

/// List every mutation patch path, repository-relative, sorted for stability.
fn patch_paths() -> Result<Vec<Utf8PathBuf>> {
    let mutations = Dir::open_ambient_dir(manifest_dir().join(MUTATIONS_DIR), ambient_authority())
        .with_context(|| format!("open {MUTATIONS_DIR}"))?;
    let mut paths = Vec::new();
    for entry_result in mutations
        .read_dir(".")
        .context("read mutations directory")?
    {
        let entry = entry_result.context("read mutations directory entry")?;
        let name = entry.file_name().context("read mutation patch name")?;
        paths.push(Utf8Path::new(MUTATIONS_DIR).join(name));
    }
    paths.sort();
    Ok(paths)
}

/// Every mutation patch must still produce a tree that compiles.
///
/// A patch that applies but does not compile is dead evidence: `cargo kani`
/// never reaches its harness, so the patch can no longer demonstrate that the
/// harness detects the seeded fault.
#[test]
#[ignore = "compiles each patched tree through the Kani frontend; run via `make test-kani-mutations`"]
fn every_patched_tree_compiles_under_denied_warnings() -> Result<()> {
    if !is_git_work_tree(manifest_dir())? {
        // cargo-mutants copies omit `.git`, so no patch can be applied there.
        // Matches `every_patch_applies_cleanly`, which skips for the same
        // reason.
        tracing::warn!(
            "skipping: source tree is not a git checkout; \
             operation=compile mutation patches under denied warnings; \
             repository={path}",
            path = manifest_dir()
        );
        return Ok(());
    }

    let target_dir = manifest_dir().join(SHARED_TARGET_DIR);
    let mut broken = Vec::new();
    let mut unreverted = Vec::new();
    for patch in patch_paths()? {
        let applied = AppliedPatch::apply(&patch)?;
        if let Some(stderr) = compile_patched_tree(&target_dir)? {
            broken.push(format!("{patch}: {stderr}"));
        }
        // Reverted through the fallible path, not left to `Drop`: a reverse
        // that fails must fail the test, or a run reports success while the
        // working tree still carries the mutation it just seeded.
        if let Err(err) = applied.revert() {
            unreverted.push(format!("{patch}: {err}"));
        }
    }
    ensure!(
        unreverted.is_empty(),
        "mutation patches could not be reverted, so the working tree still \
         carries seeded faults: {unreverted:#?}",
    );
    ensure!(
        broken.is_empty(),
        "mutation patches apply but their patched trees do not compile under \
         the Kani configuration, so the harnesses they target never run: \
         {broken:#?}; reseed each fault in place — swap an expression rather \
         than deleting a statement — so every helper stays referenced and \
         every `mut` binding is still reassigned",
    );
    Ok(())
}
