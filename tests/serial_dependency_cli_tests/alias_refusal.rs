//! Public-CLI regressions for refusing aliased dyndep directories.
//!
//! Each case stages `.netsuke/dyndep` as a symbolic link to a victim directory
//! holding cache-shaped files, then asserts that the command refuses the alias
//! and leaves the victim intact.

use super::{SERIAL_MANIFEST, cli_workspace, run_netsuke};
use anyhow::{Result, ensure};
use std::path::Path;
use tempfile::TempDir;

const SIDECAR_FREE_MANIFEST: &str = concat!(
    "netsuke_version: '1.0.0'\n",
    "targets:\n",
    "  - name: artifact.txt\n",
    "    command: touch artifact.txt\n",
    "defaults: [artifact.txt]\n",
);

/// Stage a workspace whose `.netsuke/dyndep` is a symlink to `victim/`.
///
/// The victim directory holds a sidecar-shaped file and a stale temporary
/// file, so a command that follows the alias would treat both as disposable
/// generated state.
fn aliased_dyndep_workspace() -> Result<(TempDir, std::path::PathBuf)> {
    let (directory, _root) = cli_workspace(SERIAL_MANIFEST)?;
    let victim = directory.path().join("victim");
    test_support::fs::create_dir(&victim)?;
    test_support::fs::write(victim.join("keep.dd"), "keep")?;
    test_support::fs::write(victim.join("stale.tmp"), "stale")?;
    test_support::fs::create_dir(directory.path().join(".netsuke"))?;
    test_support::fs::symlink("../victim", directory.path().join(".netsuke/dyndep"))?;
    Ok((directory, victim))
}

/// Assert that a command refused the alias and left the victim files intact.
fn assert_alias_refused(output: &std::process::Output, victim: &Path) -> Result<()> {
    ensure!(
        !output.status.success(),
        "the command must reject a dyndep directory symlink"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        stderr.contains("symbolic link") && stderr.contains(".netsuke/dyndep"),
        "stderr must name the aliased component: {stderr}"
    );
    ensure!(
        test_support::fs::exists(victim.join("keep.dd")),
        "the victim sidecar must survive"
    );
    ensure!(
        test_support::fs::exists(victim.join("stale.tmp")),
        "the victim temporary file must survive"
    );
    Ok(())
}

#[test]
fn build_rejects_a_dyndep_directory_symlink() -> Result<()> {
    let (directory, victim) = aliased_dyndep_workspace()?;

    let output = run_netsuke(&directory, &["--locale", "en-US", "-j", "3", "build"])?;

    assert_alias_refused(&output, &victim)
}

#[test]
fn clean_rejects_a_dyndep_directory_symlink() -> Result<()> {
    let (directory, victim) = aliased_dyndep_workspace()?;

    let output = run_netsuke(&directory, &["--locale", "en-US", "-j", "3", "clean"])?;

    assert_alias_refused(&output, &victim)
}

/// A sidecar-free bundle never opens the dyndep directory before Ninja runs,
/// and `clean` prunes only after `ninja -t clean`. This pins that ordering so
/// the users' guide can state the caveat honestly: the alias is refused and
/// the victim survives, but the declared output is already gone.
#[test]
fn sidecar_free_clean_refuses_the_alias_after_ninja_removes_outputs() -> Result<()> {
    let (directory, _root) = cli_workspace(SIDECAR_FREE_MANIFEST)?;
    let build = run_netsuke(&directory, &["-j", "3", "build"])?;
    ensure!(
        build.status.success(),
        "prepare the sidecar-free build output: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = directory.path().join("artifact.txt");
    ensure!(
        test_support::fs::exists(&output),
        "the build must create its declared output"
    );

    // A sidecar-free build never creates `.netsuke`: retention opens the
    // directory only when it already exists. Stage the alias from scratch.
    test_support::fs::create_dir(directory.path().join(".netsuke"))?;
    let victim = directory.path().join("victim");
    test_support::fs::create_dir(&victim)?;
    test_support::fs::write(victim.join("keep.dd"), "keep")?;
    test_support::fs::write(victim.join("stale.tmp"), "stale")?;
    test_support::fs::symlink("../victim", directory.path().join(".netsuke/dyndep"))?;

    let clean = run_netsuke(&directory, &["--locale", "en-US", "-j", "3", "clean"])?;

    let stderr = String::from_utf8_lossy(&clean.stderr);
    ensure!(
        !clean.status.success(),
        "clean must reject the dyndep alias: {stderr}"
    );
    ensure!(
        stderr.contains("symbolic link") && stderr.contains(".netsuke/dyndep"),
        "stderr must name the aliased component: {stderr}"
    );
    ensure!(
        !test_support::fs::exists(&output),
        "ninja -t clean runs before retention, so the declared output is already removed"
    );
    ensure!(
        test_support::fs::exists(victim.join("keep.dd")),
        "the victim sidecar must survive"
    );
    ensure!(
        test_support::fs::exists(victim.join("stale.tmp")),
        "the victim temporary file must survive"
    );
    Ok(())
}
