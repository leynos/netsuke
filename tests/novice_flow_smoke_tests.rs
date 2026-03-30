//! Smoke tests for newcomer-facing CLI flows.

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use std::path::Path;
use tempfile::{TempDir, tempdir};
use test_support::check_ninja;
use test_support::fluent::normalize_fluent_isolates;

struct CommandOutput {
    stdout: String,
    stderr: String,
    success: bool,
}

fn run_netsuke(
    current_dir: &Path,
    args: &[&str],
    ninja_env: Option<&Path>,
) -> Result<CommandOutput> {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("netsuke");
    cmd.current_dir(current_dir).env("PATH", "").args(args);
    if let Some(path) = ninja_env {
        cmd.env(netsuke::runner::NINJA_ENV, path);
    }
    let output = cmd.output().context("run netsuke smoke command")?;
    Ok(CommandOutput {
        stdout: normalize_fluent_isolates(&String::from_utf8_lossy(&output.stdout)),
        stderr: normalize_fluent_isolates(&String::from_utf8_lossy(&output.stderr)),
        success: output.status.success(),
    })
}

fn setup_minimal_workspace(context: &str) -> Result<TempDir> {
    let temp = tempdir().with_context(|| format!("create temp dir for {context}"))?;
    let manifest = temp.path().join("Netsukefile");
    std::fs::copy("tests/data/minimal.yml", &manifest)
        .with_context(|| format!("copy minimal manifest to {}", manifest.display()))?;
    Ok(temp)
}

fn assert_contains_all(haystack: &str, fragments: &[&str], label: &str) -> Result<()> {
    for fragment in fragments {
        ensure!(
            haystack.contains(fragment),
            "expected {label} to contain '{fragment}', got:\n{haystack}"
        );
    }
    Ok(())
}

#[test]
fn first_run_without_args_succeeds_in_minimal_workspace() -> Result<()> {
    let workspace = setup_minimal_workspace("novice smoke first run")?;
    let (_ninja_dir, ninja_path) = check_ninja::fake_ninja_check_build_file()?;

    let output = run_netsuke(workspace.path(), &[], Some(ninja_path.as_path()))?;

    ensure!(
        output.success,
        "expected bare netsuke invocation to succeed"
    );
    assert_contains_all(&output.stderr, &["Stage 6/6", "Build complete."], "stderr")?;
    Ok(())
}

#[test]
fn missing_manifest_error_matches_documented_guidance() -> Result<()> {
    let workspace = tempdir().context("create temp dir for missing manifest smoke test")?;

    let output = run_netsuke(workspace.path(), &[], None)?;

    ensure!(!output.success, "expected bare netsuke invocation to fail");
    assert_contains_all(
        &output.stderr,
        &[
            "Manifest 'Netsukefile' not found in the current directory.",
            "Ensure the manifest exists or pass `--file` with the correct path.",
        ],
        "stderr",
    )?;
    Ok(())
}

#[rstest]
#[case::flag(&["--help"])]
#[case::subcommand(&["help"])]
fn help_entry_points_are_novice_friendly(#[case] args: &[&str]) -> Result<()> {
    let output = run_netsuke(Path::new("."), args, None)?;

    ensure!(output.success, "expected help entry point to succeed");
    assert_contains_all(
        &output.stdout,
        &[
            "Netsuke transforms YAML + Jinja manifests into reproducible Ninja graphs",
            "build     Build targets defined in the manifest (default).",
            "clean     Remove build artefacts via Ninja.",
            "graph     Emit the dependency graph in DOT format.",
            "manifest  Write the generated Ninja manifest without running Ninja.",
        ],
        "stdout",
    )?;
    Ok(())
}

#[test]
fn localized_help_still_flows_through_cli_localization() -> Result<()> {
    let output = run_netsuke(Path::new("."), &["--locale", "es-ES", "--help"], None)?;

    ensure!(output.success, "expected localized help to succeed");
    assert_contains_all(
        &output.stdout,
        &[
            "Netsuke transforma manifiestos YAML + Jinja en grafos Ninja reproducibles",
            "build     Compila objetivos definidos en el manifiesto (predeterminado).",
        ],
        "stdout",
    )?;
    Ok(())
}
