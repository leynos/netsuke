//! Snapshot and executable-Ninja coverage for conditional action dependencies.
//!
//! Keeps the conditional dependency fixture and its real-Ninja validation
//! isolated from the general snapshot cases while sharing their command runner.

use super::run_ok;
use anyhow::{Context, Result, ensure};
use insta::{Settings, assert_snapshot};
use netsuke::{ir::BuildGraph, manifest, ninja_gen};
use std::{
    fs as std_fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
use tempfile::{TempDir, tempdir};
use test_support::ensure_binaries_available;

/// Snapshot and execute the selected conditional-action dependency graph.
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

/// Describe the expected Ninja dependency classes for a generated output.
struct ExpectedNinjaEdge<'a> {
    output: &'a str,
    input: &'a str,
    implicit_deps: &'a str,
    order_only_deps: &'a str,
}

/// Assert that a generated edge retains explicit, implicit, and order-only inputs.
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

/// Validate the generated conditional Ninja file with a real Ninja installation.
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

    let (dir, build_file) = prepare_conditional_ninja_workspace(ninja_content)?;
    assert_conditional_ninja_selection(&dir, &build_file)?;
    mark_conditional_ninja_output_up_to_date(&dir)?;
    assert_conditional_ninja_no_op(&dir, &build_file)
}

/// Create the files required to run the conditional Ninja fixture.
fn prepare_conditional_ninja_workspace(ninja_content: &str) -> Result<(TempDir, PathBuf)> {
    let dir = tempdir().context("create temp dir for conditional Ninja validation")?;
    let build_file = dir.path().join("build.ninja");
    std_fs::write(&build_file, ninja_content)
        .with_context(|| format!("write Ninja file to {}", build_file.display()))?;
    for relative_path in ["src/target.in", "include/fallback.h", "order/target.stamp"] {
        let dependency_path = dir.path().join(relative_path);
        let parent = dependency_path
            .parent()
            .context("dependency path should have parent")?;
        std_fs::create_dir_all(parent)
            .with_context(|| format!("create dependency directory {}", parent.display()))?;
        std_fs::write(&dependency_path, "")
            .with_context(|| format!("write Ninja dependency {}", dependency_path.display()))?;
    }

    Ok((dir, build_file))
}

/// Run Ninja with the conditional fixture as its build file.
fn run_conditional_ninja(dir: &TempDir, build_file: &Path, args: &[&str]) -> Result<String> {
    let mut cmd = Command::new("ninja");
    cmd.arg("-f").arg(build_file).args(args);
    cmd.current_dir(dir.path());
    run_ok(&mut cmd)
}

/// Confirm that Ninja selects the fallback branch and schedules its command.
fn assert_conditional_ninja_selection(dir: &TempDir, build_file: &Path) -> Result<()> {
    run_conditional_ninja(dir, build_file, &["-t", "query", "fallback-alpha"])?;
    run_conditional_ninja(dir, build_file, &["-t", "query", "out/fallback"])?;
    let first = run_conditional_ninja(dir, build_file, &["-n", "out/fallback"])?;
    ensure!(
        first.contains("echo fallback"),
        "expected dry run to reach selected target, got:\n{first}"
    );

    Ok(())
}

/// Mark the fallback output newer than its order-only dependency.
fn mark_conditional_ninja_output_up_to_date(dir: &TempDir) -> Result<()> {
    let output = dir.path().join("out/fallback");
    let output_dir = output.parent().context("output path should have parent")?;
    std_fs::create_dir_all(output_dir)
        .with_context(|| format!("create output directory {}", output_dir.display()))?;
    std_fs::write(&output, "")
        .with_context(|| format!("write up-to-date output {}", output.display()))?;
    let latest_dependency = dir.path().join("order/target.stamp");
    let output_modified = std_fs::metadata(&latest_dependency)
        .with_context(|| format!("stat dependency {}", latest_dependency.display()))?
        .modified()
        .context("read dependency modification time")?
        + Duration::from_secs(1);
    std_fs::File::options()
        .write(true)
        .open(&output)
        .with_context(|| format!("open output {}", output.display()))?
        .set_times(std_fs::FileTimes::new().set_modified(output_modified))
        .with_context(|| format!("set output time {}", output.display()))?;

    Ok(())
}

/// Confirm that the prepared conditional output has no pending Ninja work.
fn assert_conditional_ninja_no_op(dir: &TempDir, build_file: &Path) -> Result<()> {
    run_conditional_ninja(dir, build_file, &["out/fallback"])?;
    let second = run_conditional_ninja(dir, build_file, &["-n", "out/fallback"])?;
    ensure!(
        second.contains("no work to do"),
        "expected no-op second pass, got:\n{second}"
    );
    Ok(())
}
