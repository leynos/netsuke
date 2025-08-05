//! End-to-end validation of Ninja file generation.
//!
//! These tests generate a Ninja file from a manifest, snapshot the
//! output using `insta`, and validate it with the real `ninja`
//! executable. The manifest uses a simple TOUCH rule so the build is
//! fast and deterministic.

use insta::{Settings, assert_snapshot};
use netsuke::{ir::BuildGraph, manifest, ninja_gen};
use std::{fs, process::Command};
use tempfile::tempdir;

fn run_ok(cmd: &mut Command) -> String {
    let out = cmd.output().expect("should spawn command");
    assert!(
        out.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("stdout utf8")
}

#[test]
fn touch_manifest_ninja_validation() {
    let ninja_check = Command::new("ninja").arg("--version").output();
    if ninja_check.is_err() || !ninja_check.as_ref().expect("spawn ninja").status.success() {
        eprintln!("skipping test: ninja must be installed for integration tests");
        return;
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

    let manifest = manifest::from_str(manifest_yaml).expect("parse manifest");
    let ir = BuildGraph::from_manifest(&manifest).expect("ir generation");
    let ninja_content = ninja_gen::generate(&ir);

    let mut settings = Settings::new();
    settings.set_snapshot_path(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/snapshots/ninja"
    ));
    settings.bind(|| {
        assert_snapshot!("touch_manifest_ninja", ninja_content);
    });

    let dir = tempdir().expect("tempdir");
    let build_file = dir.path().join("build.ninja");
    fs::write(&build_file, &ninja_content).expect("write ninja");
    fs::create_dir_all(dir.path().join("in")).expect("dir");
    fs::write(dir.path().join("in/a"), "").expect("input");

    let ninja_cmd = |args: &[&str]| {
        let mut cmd = Command::new("ninja");
        cmd.arg("-f").arg(&build_file).args(args);
        cmd.current_dir(&dir);
        run_ok(&mut cmd)
    };

    let _ = ninja_cmd(&["-t", "rules"]);
    let _ = ninja_cmd(&["-t", "targets", "all"]);
    let _ = ninja_cmd(&["-t", "query", "out/a"]);

    let _ = ninja_cmd(&["-w", "phonycycle=err", "-d", "stats"]);
    let second = ninja_cmd(&["-n", "-d", "explain", "-v"]);
    assert!(
        second.contains("no work to do"),
        "expected no-op second pass, got:\n{second}"
    );
}
