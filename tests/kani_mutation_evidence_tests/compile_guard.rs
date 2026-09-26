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
//! # The tree it patches is a sandbox, not the checkout
//!
//! Patches are applied to an isolated copy of the working tree, exported by
//! [`sandbox`], rather than to the checkout the test is running in: reverting
//! happens on the normal path, but it cannot be what the developer's checkout
//! depends on. [`sandbox`] gives the mechanism, and why a timed-out run can
//! strand a mutation it never reverts. The parent contract
//! `every_patch_applies_cleanly` reads the working tree too, so the two agree
//! on which tree they describe.
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

mod sandbox;

use sandbox::Sandbox;

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

/// Apply `patch` to the sandbox, then revert it — explicitly on the normal
/// path, and through `Drop` when unwinding.
///
/// Both paths matter and they are not interchangeable. The explicit
/// [`Self::revert`] makes a failed reverse a test failure, so a run cannot
/// report success with a seeded mutation left behind; `Drop` covers the panic
/// — or early return from a failed assertion — that would otherwise strand a
/// mutation for every later patch to trip over.
///
/// The revert is now a tidiness step rather than a safety one. What it
/// protects is the sandbox itself, which the next `create` replaces wholesale,
/// so a mutation that survives this run cannot reach the developer's checkout.
struct AppliedPatch<'a> {
    /// Repository-relative path of the patch that was applied.
    ///
    /// This names the patch within the *sandbox*, which is what `git apply`
    /// resolves against when the sandbox is the working directory.
    patch: &'a Utf8Path,
    /// The sandboxed tree the patch is applied to, and its isolation env.
    ///
    /// Both halves are needed by every `git` call this type makes: the tree
    /// is the working directory, and the env is what stops `git` walking up
    /// from it into the real checkout.
    sandbox: &'a Sandbox,
    /// True while the patch is still applied to the sandbox.
    ///
    /// Cleared by [`Self::revert`] so the `Drop` fallback does not attempt a
    /// second reverse, which would fail and log a spurious error on every
    /// successful patch.
    applied: bool,
}

impl<'a> AppliedPatch<'a> {
    /// Apply `patch` to the sandbox, failing when it does not apply.
    fn apply(patch: &'a Utf8Path, sandbox: &'a Sandbox) -> Result<Self> {
        run_git_apply(["apply", patch.as_str()], patch, sandbox, "apply")?;
        Ok(Self {
            patch,
            sandbox,
            applied: true,
        })
    }

    /// Revert the patch, propagating a failure to the caller.
    ///
    /// Reverting here rather than relying on `Drop` is what stops a failed
    /// reverse from passing quietly: without it the test returns `Ok(())` and
    /// the sandbox keeps a mutation nothing reports, which would then be
    /// compiled into every later patch's tree.
    ///
    /// `applied` is cleared only when the reverse actually succeeded, so a
    /// failure leaves the flag set and `Drop` still attempts the reverse as
    /// the run unwinds. Clearing it unconditionally would disable that retry
    /// at the one moment it is wanted, turning a recoverable failure into a
    /// lingering mutation.
    fn revert(mut self) -> Result<()> {
        let outcome = run_git_apply(
            ["apply", "--reverse", self.patch.as_str()],
            self.patch,
            self.sandbox,
            "reverse",
        );
        if outcome.is_ok() {
            self.applied = false;
        }
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
            self.sandbox,
            "reverse",
        ) {
            // A panic is already unwinding when this runs, so the failure is
            // reported rather than raised: replacing the panic's message with
            // a revert error would hide the defect that caused it.
            tracing::error!("failed to revert {patch}: {err}", patch = self.patch);
        }
    }
}

/// Run one `git apply`-family invocation against the sandbox.
///
/// The working directory is the sandbox tree, and the environment carries the
/// ceiling that keeps `git` from walking out of it, so a patch can only ever
/// be applied to the copy.
fn run_git_apply<const N: usize>(
    args: [&str; N],
    patch: &Utf8Path,
    sandbox: &Sandbox,
    action: &str,
) -> Result<()> {
    let output = Command::new("git")
        .args(args)
        .current_dir(sandbox.tree())
        .envs(sandbox.isolation_env())
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
///
/// The build runs in the sandbox, so the ceiling travels with it: `cargo kani`
/// shells out to `git` for workspace metadata, and an unceiled invocation
/// would find the real checkout instead of the copy it is compiling.
fn compile_patched_tree(sandbox: &Sandbox, target_dir: &Utf8Path) -> Result<Option<String>> {
    let output = Command::new(env!("CARGO"))
        .args(["kani", "--only-codegen", "--lib", "--all-features"])
        .current_dir(sandbox.tree())
        .envs(sandbox.isolation_env())
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

/// List every mutation patch in the sandbox, repository-relative, sorted.
///
/// The sandbox, not the working tree. `git apply` resolves the patch's path
/// inside the sandbox, so listing from the working tree would let the two
/// disagree: a patch the developer had just added would be listed and then
/// reported as missing, while one they had just deleted would still be
/// compiled. The listing is read from the same tree the content is resolved
/// from, so a patch is either present in both or in neither.
///
/// "In both" is limited to *tracked* patches. `git stash create` captures only
/// tracked content, so an untracked patch is in the working tree and absent
/// from the sandbox. [`ensure_no_untracked_patches`] rejects that state before
/// this runs, so the only patches that can be missing here are ones that do
/// not exist. A non-`.patch` entry is refused rather than skipped, as the
/// sibling contract `patch_stems` does: skipping one would leave this gate
/// green over fewer patches than the directory holds, this issue's own failure
/// mode one turn deeper.
fn patch_paths(sandbox: &Sandbox) -> Result<Vec<Utf8PathBuf>> {
    let mutations = Dir::open_ambient_dir(sandbox.tree().join(MUTATIONS_DIR), ambient_authority())
        .with_context(|| format!("open {MUTATIONS_DIR} in the sandbox"))?;
    let mut paths = Vec::new();
    for entry_result in mutations
        .read_dir(".")
        .context("read mutations directory")?
    {
        let entry = entry_result.context("read mutations directory entry")?;
        let name = entry.file_name().context("read mutation patch name")?;
        let path = Utf8Path::new(MUTATIONS_DIR).join(&name);
        ensure!(
            Utf8Path::new(&name).extension() == Some("patch"),
            "{path} is not a .patch file; every entry in {MUTATIONS_DIR} is \
             passed to `git apply`, so keep it restricted to mutation evidence",
        );
        paths.push(path);
    }
    paths.sort();
    Ok(paths)
}

/// Require that the sandbox cannot resolve a repository outside itself.
///
/// The whole premise of the sandbox is that `git apply` run inside it cannot
/// reach the developer's checkout. That premise is one environment variable
/// wide, and an unceiled `git` would apply every patch to the live tree while
/// reporting the sandbox's path, so it is asserted rather than assumed: this
/// is the one check that fails loudly when the ceiling stops working, instead
/// of the mutation quietly landing somewhere it must not.
///
/// `git rev-parse --is-inside-work-tree` exits 128 when no repository is
/// found, which is the expected outcome here rather than an error.
fn ensure_sandbox_is_isolated(sandbox: &Sandbox) -> Result<()> {
    let probe = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(sandbox.tree())
        .envs(sandbox.isolation_env())
        .output()
        .context("probe whether the sandbox resolves a repository")?;
    ensure!(
        !probe.status.success(),
        "the sandbox at {tree} resolved a Git repository, so a mutation \
         patch applied inside it could reach the working checkout. The \
         sandbox must carry no `.git` and its `GIT_CEILING_DIRECTORIES` must \
         name a path at or below itself",
        tree = sandbox.tree(),
    );
    Ok(())
}

/// Require that every mutation patch in the checkout is tracked.
///
/// The sandbox holds the captured revision, which is tracked content only, so
/// an untracked patch cannot be compiled there. Without this check the run
/// would simply not see it: the listing comes from the sandbox, so the patch
/// would vanish between the checkout that has it and the tree the gate reports
/// on, and the run would go green over evidence it never read. That is the
/// failure mode this whole module exists to catch, so it is refused rather
/// than skipped.
///
/// `--others --exclude-standard` names untracked files that are not ignored,
/// which is exactly the set `git stash create` leaves behind.
fn ensure_no_untracked_patches(manifest_dir: &Utf8Path) -> Result<()> {
    let output = Command::new("git")
        .args([
            "ls-files",
            "--others",
            "--exclude-standard",
            "--",
            MUTATIONS_DIR,
        ])
        .current_dir(manifest_dir)
        .output()
        .context("list untracked mutation patches")?;
    ensure!(
        output.status.success(),
        "git ls-files failed: {}",
        String::from_utf8_lossy(&output.stderr).trim(),
    );
    let untracked = String::from_utf8(output.stdout).context("decode untracked patch list")?;
    ensure!(
        untracked.trim().is_empty(),
        "these mutation patches are untracked, so the gate would compile \
         every *other* patch and report success without mentioning them: \
         {}\n`git add` each one, or remove it; the sandbox is exported from \
         the tracked revision and cannot carry an untracked file.",
        untracked.trim(),
    );
    Ok(())
}

/// Every mutation patch must still produce a tree that compiles.
///
/// A patch that applies but does not compile is dead evidence: `cargo kani`
/// never reaches its harness, so the patch can no longer demonstrate that the
/// harness detects the seeded fault.
///
/// The tree compiled is an isolated copy of the current revision rather than
/// the checkout the test runs in, so a run that ends without unwinding — a
/// Nextest timeout signalling the process group, which `Drop` cannot survive —
/// cannot leave a seeded mutation where a developer would find it. "Current
/// revision" is the working tree, captured by [`sandbox`], so an edit to a
/// patch is what this compiles without first being committed.
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

    ensure_no_untracked_patches(manifest_dir())?;
    let sandbox = Sandbox::create(manifest_dir())?;
    ensure_sandbox_is_isolated(&sandbox)?;
    let target_dir = manifest_dir().join(SHARED_TARGET_DIR);
    let mut broken = Vec::new();
    let mut unreverted = Vec::new();
    for patch in patch_paths(&sandbox)? {
        let applied = AppliedPatch::apply(&patch, &sandbox)?;
        if let Some(stderr) = compile_patched_tree(&sandbox, &target_dir)? {
            broken.push(format!("{patch}: {stderr}"));
        }
        // Reverted through the fallible path, not left to `Drop`: a reverse
        // that fails must fail the test, or a run reports success while the
        // sandbox still carries the mutation it just seeded. [`revert`] has
        // already given `Drop` its own attempt by the time it returns, so an
        // error here means the mutation is genuinely still applied -- and
        // every later patch would then be compiled against a tree carrying
        // it. The loop stops rather than reporting the remaining patches
        // against a contaminated tree.
        //
        // [`revert`]: AppliedPatch::revert
        if let Err(err) = applied.revert() {
            unreverted.push(format!("{patch}: {err}"));
            break;
        }
    }
    ensure!(
        unreverted.is_empty(),
        "mutation patches could not be reverted, so the sandbox still \
         carries seeded faults and the run stopped there rather than \
         compiling later patches against a contaminated tree: \
         {unreverted:#?}",
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
