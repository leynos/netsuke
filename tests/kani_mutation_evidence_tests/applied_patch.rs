//! Apply a mutation patch to the sandbox, and take it back out again.
//!
//! The patch sweep compiles a tree per patch, so each patch has to be applied
//! before its compile and reverted before the next one's. Reverting happens
//! explicitly on the normal path and through [`Drop`] when unwinding, and the
//! two paths are not interchangeable. [`AppliedPatch::revert`] makes a failed
//! reverse a test failure, so a run cannot report success with a seeded
//! mutation left behind; [`Drop`] covers the panic — or early return from a
//! failed assertion — that would otherwise strand a mutation for every later
//! patch to trip over.
//!
//! The revert is a tidiness step rather than a safety one. What it protects is
//! the sandbox itself, which the next `Sandbox::create` replaces wholesale, so
//! a mutation that survives this run cannot reach the developer's checkout.

use std::process::Command;

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;

use super::sandbox::Sandbox;

/// A patch that is currently applied to the sandbox.
pub(super) struct AppliedPatch<'a> {
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
    pub(super) fn apply(patch: &'a Utf8Path, sandbox: &'a Sandbox) -> Result<Self> {
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
    pub(super) fn revert(mut self) -> Result<()> {
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
