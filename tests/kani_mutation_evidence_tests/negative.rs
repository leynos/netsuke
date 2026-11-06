//! Prove the Kani compile gate rejects trees that do not compile.
//!
//! The patch sweep asserts that every mutation patch compiles, and that
//! assertion is only meaningful while the same compile path can fail. If
//! `--no-codegen` stopped type-checking, every patch would look healthy and
//! the gate would prove nothing. This test seeds one fault that `cargo check`
//! also sees and one behind `#[cfg(kani)]` that only the Kani frontend parses,
//! then requires the compile path to reject each one and to accept the
//! unmodified tree.

use anyhow::{Context, Result, bail, ensure};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};

use super::compile_guard::{SHARED_TARGET_DIR, compile_kani_tree, ensure_sandbox_is_isolated};
use super::discovery::{is_git_work_tree, manifest_dir};
use super::sandbox::Sandbox;

/// Seeded faults, each paired with the marker its rejection must name.
///
/// The marker is the literal in the mismatched type, so a rejection can be
/// attributed to the seeded fault rather than to an unrelated build failure.
/// The second fault is invisible to `cargo check`, which is why the gate
/// compiles through the Kani frontend at all.
const SEEDED_FAULTS: [(&str, &str); 2] = [
    (
        "\nfn seeded_ordinary_fault() -> u8 {\n    \"seeded ordinary fault\"\n}\n",
        "seeded ordinary fault",
    ),
    (
        "\n#[cfg(kani)]\nfn seeded_kani_fault() -> u8 {\n    \"seeded kani fault\"\n}\n",
        "seeded kani fault",
    ),
];

/// Require the compile path to accept a valid tree and reject seeded faults.
///
/// The unmodified tree is compiled first, so a rejection below cannot be
/// explained by a sandbox that never built. Each fault then gets a fresh
/// sandbox, because the sandbox path is fixed and re-extracting it is what
/// removes the previous fault.
///
/// # Errors
///
/// Returns an error if the sandbox cannot be prepared, if the unmodified tree
/// does not compile, or if a seeded fault compiles instead of being rejected.
#[test]
#[ignore = "compiles deliberately broken sandbox trees; run via `make test-kani-mutations`"]
fn compile_kani_tree_rejects_trees_that_do_not_compile() -> Result<()> {
    if !is_git_work_tree(manifest_dir())? {
        tracing::warn!(
            "skipping: source tree is not a git checkout; \
             operation=verify the Kani compile gate rejects broken trees; \
             repository={path}",
            path = manifest_dir()
        );
        return Ok(());
    }

    let target_dir = manifest_dir().join(SHARED_TARGET_DIR);
    let sandbox = Sandbox::create(manifest_dir())?;
    ensure_sandbox_is_isolated(&sandbox)?;
    if let Some(stderr) = compile_kani_tree(&sandbox, &target_dir)? {
        bail!("the unmodified sandbox tree failed to compile: {stderr}");
    }

    for (fault, marker) in SEEDED_FAULTS {
        let stderr = compile_seeded_fault(&target_dir, fault)?;
        ensure!(
            stderr.contains(marker),
            "a seeded fault was rejected for a reason other than the fault \
             itself: marker {marker:?} is absent from {stderr}"
        );
    }
    Ok(())
}

/// Compile a fresh sandbox carrying one seeded fault and return its stderr.
///
/// # Errors
///
/// Returns an error if the sandbox cannot be prepared, if the fault cannot be
/// written, or if the seeded tree compiles.
fn compile_seeded_fault(target: &Utf8Path, fault: &str) -> Result<String> {
    let sandbox = Sandbox::create(manifest_dir())?;
    ensure_sandbox_is_isolated(&sandbox)?;
    append_library_fault(&sandbox, fault)?;
    compile_kani_tree(&sandbox, target)?
        .with_context(|| format!("a sandbox tree carrying this seeded fault compiled: {fault}"))
}

/// Append `fault` to the sandbox library root so the next compile sees it.
///
/// The library root is the crate root of the target `cargo kani --lib`
/// compiles, so an item appended there is parsed whether or not it sits behind
/// `#[cfg(kani)]`.
///
/// # Errors
///
/// Returns an error if the sandbox tree or its library root cannot be read or
/// written.
fn append_library_fault(sandbox: &Sandbox, fault: &str) -> Result<()> {
    let tree = Dir::open_ambient_dir(sandbox.tree(), ambient_authority())
        .with_context(|| format!("open sandbox tree {}", sandbox.tree()))?;
    let library = tree
        .read_to_string("src/lib.rs")
        .context("read the sandbox library root")?;
    tree.write("src/lib.rs", format!("{library}{fault}"))
        .context("seed the fault into the sandbox library root")?;
    Ok(())
}
