//! Public-CLI contracts for dependency participation in cycle detection.

#![cfg(unix)]

mod documentation_examples;

use anyhow::{Context, Result, ensure};
use documentation_examples::manifest_workspace;
use insta::Settings;
use std::path::{Path, PathBuf};
use test_support::fluent::normalize_fluent_isolates;
use test_support::fs as test_fs;
use test_support::netsuke::{NetsukeRun, run_netsuke_in, run_netsuke_in_with_env};
use test_support::ninja::ninja_integration_workspace;
use test_support::write_exec_with_content;

/// Write the cycle manifest used by CLI scenarios into an isolated workspace.
fn write_order_only_cycle(workspace: &Path) -> Result<()> {
    test_fs::write(
        workspace.join("Netsukefile"),
        include_str!("data/order_only_cycle.yml"),
    )
    .context("write order-only cycle manifest")
}

/// Run Netsuke in the provided workspace and capture its output.
fn run_netsuke(workspace: &Path, arguments: &[&str]) -> Result<NetsukeRun> {
    run_netsuke_in(workspace, arguments).context("run Netsuke CLI")
}

/// Reject an order-only cycle before the configured Ninja executable starts.
#[test]
fn order_only_cycle_rejected_before_ninja() -> Result<()> {
    let workspace = tempfile::tempdir().context("create cycle CLI workspace")?;
    write_order_only_cycle(workspace.path())?;
    let sentinel = workspace.path().join("ninja-started");
    let fake_ninja = write_exec_with_content(
        workspace.path(),
        "fake-ninja",
        "#!/bin/sh\nset -eu\nprintf started > \"$NETSUKE_NINJA_SENTINEL\"\n",
    )
    .context("write sentinel fake Ninja")?;
    let ninja = fake_ninja
        .to_str()
        .context("fake Ninja path should be UTF-8")?;
    let sentinel_path = sentinel
        .to_str()
        .context("Ninja sentinel path should be UTF-8")?;
    let output = run_netsuke_in_with_env(
        workspace.path(),
        &["--locale", "en-US", "--no-input", "build"],
        &[
            ("NETSUKE_NINJA", ninja),
            ("NETSUKE_NINJA_SENTINEL", sentinel_path),
        ],
    )
    .context("run Netsuke build with sentinel Ninja")?;

    let stderr = normalize_fluent_isolates(&output.stderr);
    ensure!(!output.success, "cycle build should fail");
    ensure!(
        stderr.contains("Circular dependency detected")
            && stderr.contains("headers.stamp")
            && stderr.contains("tool"),
        "cycle diagnostic should name both targets: {stderr}"
    );
    ensure!(
        !sentinel.exists(),
        "Ninja must not start when IR cycle detection fails"
    );
    Ok(())
}

/// Snapshot the stable JSON diagnostic emitted for an order-only cycle.
#[test]
fn order_only_cycle_json_diagnostic() -> Result<()> {
    let workspace = tempfile::tempdir().context("create JSON cycle workspace")?;
    write_order_only_cycle(workspace.path())?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(workspace.path())
        .env("LANG", "C")
        .args(["--json", "--locale", "en-US", "--no-input", "generate"])
        .output()
        .context("run Netsuke JSON generate")?;
    ensure!(
        !output.status.success(),
        "JSON cycle generation should fail"
    );
    ensure!(
        output.stdout.is_empty(),
        "JSON failures should leave stdout empty"
    );
    let stderr = normalize_fluent_isolates(&String::from_utf8(output.stderr)?);
    ensure!(
        stderr.contains("headers.stamp") && stderr.contains("tool"),
        "JSON diagnostic should preserve the cycle path: {stderr}"
    );
    let mut document: serde_json::Value =
        serde_json::from_str(&stderr).context("parse JSON cycle diagnostic")?;
    *document
        .pointer_mut("/generator/version")
        .context("JSON diagnostic should contain the generator version")? =
        serde_json::Value::String("[version]".to_owned());
    let rendered = serde_json::to_string_pretty(&document)
        .context("render normalized JSON cycle diagnostic")?;

    let mut settings = Settings::new();
    settings.set_snapshot_path(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots"));
    settings.bind(|| insta::assert_snapshot!(rendered));
    Ok(())
}

/// Reject the order-only cycle shown in the tested users' guide example.
#[test]
fn documented_order_only_cycle_is_rejected() -> Result<()> {
    let workspace = manifest_workspace("guide-order-only-cycle-manifest")?;
    let output = run_netsuke(
        workspace.path(),
        &["--locale", "en-US", "--no-input", "generate"],
    )?;
    ensure!(!output.success, "documented order-only cycle should fail");
    let stderr = normalize_fluent_isolates(&output.stderr);
    ensure!(
        stderr.contains(
            "Circular dependency detected: [\"headers.stamp\", \"tool\", \"headers.stamp\"]."
        ),
        "documented diagnostic should render the cycle path as a list: {stderr}"
    );
    Ok(())
}

/// Build a serial aggregate whose earlier dependency relies on a later one.
#[test]
fn serial_earlier_dependency_on_later_builds() -> Result<()> {
    let workspace = match ninja_integration_workspace() {
        Ok(workspace) => workspace,
        Err(error) => {
            tracing::warn!(%error, "skipping serial dependency integration test: Ninja unavailable");
            return Ok(());
        }
    };
    test_fs::write(
        workspace.path().join("Netsukefile"),
        include_str!("data/serial_earlier_depends_on_later.yml"),
    )
    .context("write serial dependency manifest")?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(workspace.path())
        .args(["--progress", "never", "build"])
        .output()
        .context("run serial dependency build with real Ninja")?;
    ensure!(
        output.status.success(),
        "serial dependency build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        test_fs::exists(workspace.path().join("a")) && test_fs::exists(workspace.path().join("b")),
        "both serial dependencies should be produced"
    );
    let order = test_fs::read_to_string(workspace.path().join("build-order"))
        .context("read serial dependency execution order")?;
    ensure!(
        order.lines().collect::<Vec<_>>() == ["b", "a"],
        "the later dependency must run before its dependent: {order:?}"
    );
    Ok(())
}
