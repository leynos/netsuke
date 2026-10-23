//! Check CLI ordering and deterministic graph-export output.

use super::*;

/// Reject `--edges` without root JSON before inspecting the manifest path.
#[test]
fn graph_export_requires_root_json_before_manifest_loading() -> Result<()> {
    let temp = tempfile::tempdir().context("create missing-manifest workspace")?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .args(["help", "targets", "--edges"])
        .output()
        .context("run graph export without root --json")?;

    ensure!(!output.status.success(), "missing root JSON should fail");
    ensure!(
        output.stdout.is_empty(),
        "failure should leave stdout empty"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        stderr.contains("--edges option requires root-level --json")
            && !stderr.contains("not found in"),
        "the JSON requirement should fail before manifest loading: {stderr}"
    );
    Ok(())
}

/// Produce byte-identical graph JSON on repeated runs over one manifest.
#[test]
fn graph_export_is_deterministic_across_repeated_runs() -> Result<()> {
    let (temp, manifest_path, _workspace) = edge_export_workspace(EDGE_EXPORT_MANIFEST)?;
    let temp_path = Utf8Path::from_path(temp.path()).context("workspace path should be UTF-8")?;
    let first = run_edge_export(&manifest_path, temp_path)?;
    let second = run_edge_export(&manifest_path, temp_path)?;
    ensure!(
        first.status.success() && second.status.success(),
        "both exports should succeed"
    );
    ensure!(
        first.stdout == second.stdout,
        "repeated graph exports should be byte-identical"
    );
    Ok(())
}
