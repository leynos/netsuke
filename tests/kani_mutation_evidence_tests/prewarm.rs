//! Warm the Kani mutation target before the timed patch sweep.
//!
//! The first Kani frontend compile builds a cold dependency graph. Running it
//! in its own Nextest invocation leaves the second invocation's per-test and
//! whole-run budgets for the incremental patch compiles.

use anyhow::{Result, bail};

use super::compile_guard::{SHARED_TARGET_DIR, compile_kani_tree, ensure_sandbox_is_isolated};
use super::discovery::{is_git_work_tree, manifest_dir};
use super::sandbox::Sandbox;

/// Compile the unpatched sandbox to warm the target for the mutation sweep.
///
/// Uses the same fixed sandbox and target paths as the patch test, so Cargo can
/// reuse the cold dependency build in the following Nextest invocation.
///
/// # Errors
///
/// Returns an error if sandbox creation, isolation, or Kani compilation fails.
#[test]
#[ignore = "warms the shared Kani mutation target; run via `make test-kani-mutations`"]
fn prewarm_kani_mutation_target() -> Result<()> {
    if !is_git_work_tree(manifest_dir())? {
        tracing::warn!(
            "skipping: source tree is not a git checkout; \
             operation=prewarm Kani mutation compile target; \
             repository={path}",
            path = manifest_dir()
        );
        return Ok(());
    }

    let sandbox = Sandbox::create(manifest_dir())?;
    ensure_sandbox_is_isolated(&sandbox)?;
    let target_dir = manifest_dir().join(SHARED_TARGET_DIR);
    if let Some(stderr) = compile_kani_tree(&sandbox, &target_dir)? {
        bail!("the unpatched tree failed its Kani frontend warm-up: {stderr}");
    }
    Ok(())
}
