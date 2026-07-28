//! End-to-end validation of Ninja file generation.
//!
//! These tests generate a Ninja file from a manifest, snapshot the
//! output using `insta`, and validate it with the real `ninja`
//! executable. The manifest uses a simple TOUCH rule so the build is
//! fast and deterministic.
use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs_utf8::Dir};
use insta::{Settings, assert_snapshot};
use netsuke::{ir::BuildGraph, manifest, ninja_gen, stdlib::StdlibConfig};
use std::{fs, io::Write, process::Command, time::Duration};
use tempfile::tempdir;
use test_support::ensure_binaries_available;

fn run_ok(cmd: &mut Command) -> Result<String> {
    let out = cmd.output().context("failed to spawn command")?;
    let status = out.status;
    ensure!(
        status.success(),
        "command failed with status {status}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).context("stdout must be valid UTF-8")?;
    Ok(stdout)
}

#[test]
fn touch_manifest_ninja_validation() -> Result<()> {
    if let Err(err) =
        ensure_binaries_available(&[("ninja", &["--version"]), ("python3", &["--version"])])
    {
        tracing::warn!("skipping test: required binaries unavailable: {}", err);
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
    let input_file = input_dir.join("a");
    fs::write(&input_file, "")
        .with_context(|| format!("write input file {}", input_file.display()))?;

    let ninja_cmd = |args: &[&str]| {
        let mut cmd = Command::new("ninja");
        cmd.arg("-f").arg(&build_file).args(args);
        cmd.current_dir(dir.path());
        tracing::debug!(build_file = %build_file.display(), args = ?args, "running ninja");
        run_ok(&mut cmd)
    };

    ninja_cmd(&["-t", "rules"])?;
    ninja_cmd(&["-t", "targets", "all"])?;
    ninja_cmd(&["-t", "query", "out/a"])?;

    ninja_cmd(&["-w", "phonycycle=err", "-d", "stats"])?;
    let second = ninja_cmd(&["-n", "-d", "explain", "-v"])?;
    ensure!(
        second.contains("no work to do"),
        "expected no-op second pass, got:\n{second}"
    );
    Ok(())
}

#[test]
fn conditional_manifest_ninja_snapshot() -> Result<()> {
    let manifest_yaml = r#"
        netsuke_version: "1.0.0"
        rules:
          - name: touch
            command: "touch $out"
        targets:
          - name: out/{{ item }}
            sources: in/{{ item }}
            rule: touch
            foreach:
              - kept
              - skipped
            when: item != 'skipped'
    "#;

    let manifest = manifest::from_str(manifest_yaml)?;
    let ir = BuildGraph::from_manifest(&manifest)?;
    let ninja_content = ninja_gen::generate(&ir)?;

    ensure!(
        ninja_content.contains("build out/kept:") && ninja_content.contains(" in/kept"),
        "expected kept target build rule in Ninja output:\n{ninja_content}"
    );
    ensure!(
        !ninja_content.contains("skipped"),
        "skipped target should not appear in Ninja output:\n{ninja_content}"
    );

    let mut settings = Settings::new();
    settings.set_snapshot_path(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/snapshots/ninja"
    ));
    settings.bind(|| {
        assert_snapshot!("conditional_manifest_ninja", ninja_content);
    });

    Ok(())
}

#[test]
fn command_available_manifest_ninja_snapshot() -> Result<()> {
    // Pin the `command_available` resolver to an empty PATH through the
    // stdlib configuration seam, so a host or CI image with a binary named
    // like the fixture cannot flip the guard. `cwd_mode="never"` additionally
    // excludes the workspace root from the search; the absent command name
    // alone is not sufficient for determinism.
    let manifest_yaml = r#"
        netsuke_version: "1.0.0"
        actions:
          - name: preferred-action
            command: echo preferred
            when: command_available("netsuke-command-that-should-not-exist", cwd_mode="never")
          - name: fallback-action
            command: echo fallback
            when: not command_available("netsuke-command-that-should-not-exist", cwd_mode="never")
        rules:
          - name: touch
            command: "touch $out"
        targets:
          - name: out/result
            sources: in/source
            rule: touch
    "#;

    let config = StdlibConfig::from_current_dir()?.with_path_override("");
    let manifest = manifest::from_str_with_env_and_config(
        manifest_yaml,
        &manifest::process_env_reader(),
        config,
    )?;
    let ir = BuildGraph::from_manifest(&manifest)?;
    let ninja_content = ninja_gen::generate(&ir)?;

    ensure!(
        ninja_content.contains("fallback"),
        "expected fallback action in Ninja output:\n{ninja_content}"
    );
    ensure!(
        !ninja_content.contains("preferred"),
        "preferred action guarded by an absent command should not appear:\n{ninja_content}"
    );

    let mut settings = Settings::new();
    settings.set_snapshot_path(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/snapshots/ninja"
    ));
    settings.bind(|| {
        assert_snapshot!("command_available_manifest_ninja", ninja_content);
    });

    Ok(())
}

#[test]
fn multi_command_manifest_ninja_snapshot() -> Result<()> {
    let fixture_dir = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
        .context("open repository root to read tests/data/multi_command.yml")?;
    let manifest_yaml = fixture_dir
        .read_to_string("tests/data/multi_command.yml")
        .context("read tests/data/multi_command.yml")?;

    let manifest = manifest::from_str(&manifest_yaml)?;
    let ir = BuildGraph::from_manifest(&manifest)?;
    let ninja_content = ninja_gen::generate(&ir)?;

    ensure!(
        ninja_content.contains("if eval 'echo check-fmt'")
            && ninja_content.contains("if eval 'echo lint'")
            && ninja_content.contains("if eval 'echo test'")
            && ninja_content.matches("} && {").count() == 2,
        "expected the command list joined into a fail-fast chain:\n{ninja_content}"
    );
    ensure!(
        ninja_content.contains("build done:") && ninja_content.contains("build aggregate:"),
        "the multi-command rule should be referenced by both a target and an action:\n{ninja_content}"
    );

    let mut settings = Settings::new();
    settings.set_snapshot_path(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/snapshots/ninja"
    ));
    settings.bind(|| {
        assert_snapshot!("multi_command_manifest_ninja", ninja_content);
    });

    Ok(())
}

#[test]
fn implicit_deps_manifest_ninja_snapshot() -> Result<()> {
    let fixture_dir = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
        .context("open repository root to read tests/data/implicit_deps.yml")?;
    let manifest_yaml = fixture_dir
        .read_to_string("tests/data/implicit_deps.yml")
        .context("read tests/data/implicit_deps.yml")?;

    let manifest = manifest::from_str(&manifest_yaml)?;
    let ir = BuildGraph::from_manifest(&manifest)?;
    let ninja_content = ninja_gen::generate(&ir)?;

    ensure!(
        ninja_content.contains(" | "),
        "expected implicit deps (Ninja `|` syntax) in output:\n{ninja_content}"
    );

    let mut settings = Settings::new();
    settings.set_snapshot_path(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/snapshots/ninja"
    ));
    settings.bind(|| {
        assert_snapshot!("implicit_deps_manifest_ninja", ninja_content);
    });

    Ok(())
}

#[test]
fn conditional_action_deps_ninja_snapshot() -> Result<()> {
    let manifest = manifest::from_path("tests/data/conditional_action_deps.yml")?;
    let ir = BuildGraph::from_manifest(&manifest)?;
    let ninja_content = ninja_gen::generate(&ir)?;

    assert_dependency_classes(
        &ninja_content,
        &ExpectedNinjaEdge {
            output: "fallback-alpha",
            input: "src/alpha.in",
            implicit_deps: "build/alpha.o shared/action.cfg",
            order_only_deps: "order/alpha.stamp",
        },
    )?;
    assert_dependency_classes(
        &ninja_content,
        &ExpectedNinjaEdge {
            output: "fallback-beta",
            input: "src/beta.in",
            implicit_deps: "build/beta.o shared/action.cfg",
            order_only_deps: "order/beta.stamp",
        },
    )?;
    assert_dependency_classes(
        &ninja_content,
        &ExpectedNinjaEdge {
            output: "out/fallback",
            input: "src/target.in",
            implicit_deps: "include/fallback.h",
            order_only_deps: "order/target.stamp",
        },
    )?;
    ensure!(
        !ninja_content.contains("preferred"),
        "filtered branches should not appear in Ninja output:\n{ninja_content}"
    );

    let mut settings = Settings::new();
    settings.set_snapshot_path(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/snapshots/ninja"
    ));
    settings.bind(|| {
        assert_snapshot!("conditional_action_deps_ninja", ninja_content);
    });

    validate_conditional_ninja(&ninja_content)
}

struct ExpectedNinjaEdge<'a> {
    output: &'a str,
    input: &'a str,
    implicit_deps: &'a str,
    order_only_deps: &'a str,
}

fn assert_dependency_classes(ninja_content: &str, expected: &ExpectedNinjaEdge<'_>) -> Result<()> {
    let build_line = ninja_content
        .lines()
        .find(|line| line.starts_with(&format!("build {}:", expected.output)))
        .with_context(|| format!("expected build line for {}", expected.output))?;
    ensure!(
        build_line.contains(&format!(
            " {} | {} || {}",
            expected.input, expected.implicit_deps, expected.order_only_deps
        )),
        "unexpected dependency classes for {}: {build_line}",
        expected.output
    );
    Ok(())
}

fn validate_conditional_ninja(ninja_content: &str) -> Result<()> {
    if let Err(err) = ensure_binaries_available(&[("ninja", &["--version"])]) {
        tracing::warn!("skipping real Ninja validation: {}", err);
        writeln!(
            std::io::stderr().lock(),
            "skipping real Ninja validation: {err}"
        )
        .context("write real Ninja skip reason")?;
        return Ok(());
    }

    let dir = tempdir().context("create temp dir for conditional Ninja validation")?;
    let build_file = dir.path().join("build.ninja");
    fs::write(&build_file, ninja_content)
        .with_context(|| format!("write Ninja file to {}", build_file.display()))?;
    for relative_path in ["src/target.in", "include/fallback.h", "order/target.stamp"] {
        let dependency_path = dir.path().join(relative_path);
        let parent = dependency_path
            .parent()
            .context("dependency path should have parent")?;
        fs::create_dir_all(parent)
            .with_context(|| format!("create dependency directory {}", parent.display()))?;
        fs::write(&dependency_path, "")
            .with_context(|| format!("write Ninja dependency {}", dependency_path.display()))?;
    }

    let ninja_cmd = |args: &[&str]| {
        let mut cmd = Command::new("ninja");
        cmd.arg("-f").arg(&build_file).args(args);
        cmd.current_dir(dir.path());
        run_ok(&mut cmd)
    };
    ninja_cmd(&["-t", "query", "fallback-alpha"])?;
    ninja_cmd(&["-t", "query", "out/fallback"])?;
    let first = ninja_cmd(&["-n", "out/fallback"])?;
    ensure!(
        first.contains("echo fallback"),
        "expected dry run to reach selected target, got:\n{first}"
    );

    let output = dir.path().join("out/fallback");
    let output_dir = output.parent().context("output path should have parent")?;
    fs::create_dir_all(output_dir)
        .with_context(|| format!("create output directory {}", output_dir.display()))?;
    fs::write(&output, "")
        .with_context(|| format!("write up-to-date output {}", output.display()))?;
    let latest_dependency = dir.path().join("order/target.stamp");
    let output_modified = fs::metadata(&latest_dependency)
        .with_context(|| format!("stat dependency {}", latest_dependency.display()))?
        .modified()
        .context("read dependency modification time")?
        + Duration::from_secs(1);
    fs::File::options()
        .write(true)
        .open(&output)
        .with_context(|| format!("open output {}", output.display()))?
        .set_times(fs::FileTimes::new().set_modified(output_modified))
        .with_context(|| format!("set output time {}", output.display()))?;
    ninja_cmd(&["out/fallback"])?;
    let second = ninja_cmd(&["-n", "out/fallback"])?;
    ensure!(
        second.contains("no work to do"),
        "expected no-op second pass, got:\n{second}"
    );
    Ok(())
}
