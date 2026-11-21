//! Public-CLI regressions for refusing access to reserved dyndep state.
//!
//! Most cases stage `.netsuke/dyndep` as a symbolic link to a victim directory
//! holding cache-shaped files, then assert that the command refuses the alias
//! and leaves the victim intact. One case declares an absolute output inside
//! the namespace instead, which only the build's working-directory resolution
//! can refuse, and asserts the refusal runs before any sidecar is published.

use super::{SERIAL_MANIFEST, cli_workspace, run_netsuke};
use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
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

/// Build a serial manifest that also declares `alias` as an output.
///
/// The serial list makes the bundle require sidecars, so a build that
/// published before refusing would leave `.netsuke/dyndep` behind.
fn manifest_declaring(alias: &str) -> String {
    format!(
        concat!(
            "netsuke_version: '1.0.0'\n",
            "targets:\n",
            "  - name: first\n",
            "    command: touch first\n",
            "  - name: second\n",
            "    command: touch second\n",
            "  - name: all\n",
            "    dependency_order: serial\n",
            "    deps: [first, second]\n",
            "    command: touch all\n",
            "  - name: '{alias}'\n",
            "    command: touch unused\n",
            "defaults: [all]\n",
        ),
        alias = alias
    )
}

/// Refuse a declared path inside the sidecar namespace before publishing.
///
/// Generation cannot judge an absolute spelling, so the build must resolve it.
/// That refusal has to run before publication and pruning: a build that
/// published first would leave generated sidecars behind even though it
/// refused the manifest.
#[test]
fn build_refuses_a_declared_dyndep_output_before_publishing() -> Result<()> {
    let (directory, root) = cli_workspace(SERIAL_MANIFEST)?;
    let alias = Utf8PathBuf::from_path_buf(directory.path().join(".netsuke/dyndep/declared.dd"))
        .map_err(|path| anyhow::anyhow!("alias path is not UTF-8: {}", path.display()))?;
    root.write("Netsukefile", manifest_declaring(alias.as_str()))
        .context("rewrite the manifest")?;

    let output = run_netsuke(&directory, &["--locale", "en-US", "-j", "3", "build"])?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        !output.status.success(),
        "a declared path inside .netsuke/dyndep must be refused: {stderr}"
    );
    ensure!(
        stderr.contains("reserved"),
        "the refusal must report the reservation: {stderr}"
    );
    ensure!(
        !test_support::fs::exists(directory.path().join(".netsuke/dyndep")),
        "a refused build must not publish dyndep sidecars"
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
