//! End-to-end validation of Ninja file generation.
//!
//! These tests generate a Ninja file from a manifest, snapshot the
//! output using `insta`, and validate it with the real `ninja`
//! executable. The manifest uses a simple TOUCH rule so the build is
//! fast and deterministic.

use anyhow::{Context, Result, ensure};
use insta::{Settings, assert_snapshot};
use netsuke::{ir::BuildGraph, manifest, ninja_gen};
use std::{fs, process::Command};
use tempfile::tempdir;
use test_support::ensure_binaries_available;

fn run_ok(cmd: &mut Command) -> Result<String> {
    let out = cmd.output().context("failed to spawn command")?;
    ensure!(
        out.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).context("stdout must be valid UTF-8")?;
    Ok(stdout)
}

#[test]
fn touch_manifest_ninja_validation() -> Result<()> {
    if let Err(failures) =
        ensure_binaries_available(&[("ninja", &["--version"]), ("python3", &["--version"])])
    {
        tracing::warn!(
            "skipping test: required binaries unavailable: {}",
            failures.join(", ")
        );
        return Ok(());
    }
    let manifest_yaml = r#"
        netsuke_version: "1.0.0"
        rules:
          - name: touch
            command: "python3 -c 'import os,sys; open(sys.argv[1],\"a\").close()' $out"
        targets:
          - name: out/a
            sources: in/a
            rule: touch
    "#;

    let manifest = manifest::from_str(manifest_yaml)?;
    let ir = BuildGraph::from_manifest(&manifest)?;
    let ninja_content = ninja_gen::generate(&ir)?;

    let mut settings = Settings::new();
    settings.set_snapshot_path(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/snapshots/ninja"
    ));
    settings.bind(|| {
        assert_snapshot!("touch_manifest_ninja", ninja_content);
    });

    let dir = tempdir().context("create temp dir for ninja snapshot")?;
    let build_file = dir.path().join("build.ninja");
    fs::write(&build_file, &ninja_content)
        .with_context(|| format!("write ninja file to {}", build_file.display()))?;
    let input_dir = dir.path().join("in");
    fs::create_dir_all(&input_dir)
        .with_context(|| format!("create directory {}", input_dir.display()))?;
    fs::write(dir.path().join("in/a"), "")
        .with_context(|| format!("write input file in {}", input_dir.display()))?;

    let ninja_cmd = |args: &[&str]| {
        let mut cmd = Command::new("ninja");
        cmd.arg("-f").arg(&build_file).args(args);
        cmd.current_dir(&dir);
        tracing::debug!(build_file = %build_file.display(), args = ?args, "running ninja");
        run_ok(&mut cmd)
    };

    let _ = ninja_cmd(&["-t", "rules"])?;
    let _ = ninja_cmd(&["-t", "targets", "all"])?;
    let _ = ninja_cmd(&["-t", "query", "out/a"])?;

    let _ = ninja_cmd(&["-w", "phonycycle=err", "-d", "stats"])?;
    let second = ninja_cmd(&["-n", "-d", "explain", "-v"])?;
    ensure!(
        second.contains("no work to do"),
        "expected no-op second pass, got:\n{second}"
    );
    Ok(())
}
