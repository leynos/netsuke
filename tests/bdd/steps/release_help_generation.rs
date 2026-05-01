//! Step definitions for release-help generation workflow scenarios.

use crate::bdd::fixtures::TestWorld;
use anyhow::{Context, Result, ensure};
use rstest_bdd_macros::{given, then};
use std::{fs, path::PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_repo_file(path: &str) -> Result<String> {
    let file_path = repo_root().join(path);
    fs::read_to_string(&file_path)
        .with_context(|| format!("read repository file {}", file_path.display()))
}

const fn observe_world(_world: &TestWorld) {}

#[given("the release help workflow files are available")]
fn release_help_workflow_files_are_available(world: &TestWorld) -> Result<()> {
    observe_world(world);
    for path in [
        ".github/workflows/build-and-package.yml",
        ".github/release-staging.toml",
        "scripts/generate-release-help.sh",
    ] {
        let contents = read_repo_file(path)?;
        ensure!(!contents.is_empty(), "{path} should not be empty");
    }
    Ok(())
}

#[then("the build workflow installs cargo-orthohelp before generating help")]
fn build_workflow_installs_orthohelp_before_generating(world: &TestWorld) -> Result<()> {
    observe_world(world);
    let workflow = read_repo_file(".github/workflows/build-and-package.yml")?;
    let install = workflow
        .find("cargo install cargo-orthohelp --version 0.8.0 --locked")
        .context("cargo-orthohelp install step should exist")?;
    let generate = workflow
        .find("scripts/generate-release-help.sh")
        .context("release help generation step should exist")?;
    ensure!(
        install < generate,
        "cargo-orthohelp should be installed before help generation"
    );
    Ok(())
}

#[then("the build workflow generates help under target/orthohelp")]
fn build_workflow_generates_help_under_target_orthohelp(world: &TestWorld) -> Result<()> {
    observe_world(world);
    let workflow = read_repo_file(".github/workflows/build-and-package.yml")?;
    ensure!(
        workflow.contains("\"target/orthohelp/${{ inputs.target }}/release\""),
        "release help output root should be target/orthohelp"
    );
    Ok(())
}

#[then("release staging declares the orthohelp manual page")]
fn release_staging_declares_the_orthohelp_manual_page(world: &TestWorld) -> Result<()> {
    observe_world(world);
    let staging = read_repo_file(".github/release-staging.toml")?;
    ensure!(
        staging.contains("target/orthohelp/{target}/release/man/man1/{bin_name}.1"),
        "release staging should declare the orthohelp man page"
    );
    Ok(())
}

#[then("Linux packaging consumes the staged manual page")]
fn linux_packaging_consumes_the_staged_manual_page(world: &TestWorld) -> Result<()> {
    observe_world(world);
    let workflow = read_repo_file(".github/workflows/build-and-package.yml")?;
    ensure!(
        workflow.contains("man-paths: ${{ steps.stage_paths.outputs.man_path }}"),
        "Linux package generation should use staged man_path"
    );
    Ok(())
}

#[then("the build workflow generates PowerShell help for Windows targets")]
fn build_workflow_generates_powershell_help_for_windows(world: &TestWorld) -> Result<()> {
    observe_world(world);
    let script = read_repo_file("scripts/generate-release-help.sh")?;
    ensure!(
        script.contains("target_is_windows"),
        "script should gate PowerShell generation on Windows targets"
    );
    ensure!(
        script.contains("--format ps"),
        "script should invoke cargo orthohelp for PowerShell output"
    );
    ensure!(
        script.contains("--ps-module-name \"$module_name\""),
        "script should pass the configured PowerShell module name"
    );
    Ok(())
}

#[then("release staging declares the Windows PowerShell help files")]
fn release_staging_declares_the_windows_powershell_help_files(world: &TestWorld) -> Result<()> {
    observe_world(world);
    let staging = read_repo_file(".github/release-staging.toml")?;
    for path in [
        "target/orthohelp/{target}/release/powershell/Netsuke/Netsuke.psm1",
        "target/orthohelp/{target}/release/powershell/Netsuke/Netsuke.psd1",
        "target/orthohelp/{target}/release/powershell/Netsuke/en-US/Netsuke-help.xml",
        "target/orthohelp/{target}/release/powershell/Netsuke/en-US/about_Netsuke.help.txt",
    ] {
        ensure!(
            staging.contains(path),
            "release staging should declare {path}"
        );
    }
    Ok(())
}

#[then("invalid SOURCE_DATE_EPOCH handling falls back to the epoch date with a warning")]
fn invalid_source_date_epoch_falls_back_to_epoch(world: &TestWorld) -> Result<()> {
    observe_world(world);
    let script = read_repo_file("scripts/generate-release-help.sh")?;
    ensure!(
        script.contains("fallback_date=\"1970-01-01\""),
        "script should use 1970-01-01 fallback date"
    );
    ensure!(
        script.contains("Invalid SOURCE_DATE_EPOCH"),
        "script should warn for invalid SOURCE_DATE_EPOCH values"
    );
    Ok(())
}

#[then("missing help outputs fail with release help errors")]
fn missing_help_outputs_fail_with_release_help_errors(world: &TestWorld) -> Result<()> {
    observe_world(world);
    let script = read_repo_file("scripts/generate-release-help.sh")?;
    ensure!(
        script.contains("::error title=Release help missing::"),
        "script should emit GitHub Actions release-help errors"
    );
    ensure!(
        script.contains("manual page was not generated"),
        "script should fail when the generated man page is missing"
    );
    ensure!(
        script.contains("PowerShell MAML help was not generated"),
        "script should fail when generated MAML help is missing"
    );
    Ok(())
}

#[then("the workflow no longer references build.rs generated help paths")]
fn workflow_no_longer_references_build_rs_help_paths(world: &TestWorld) -> Result<()> {
    observe_world(world);
    let workflow = read_repo_file(".github/workflows/build-and-package.yml")?;
    let staging = read_repo_file(".github/release-staging.toml")?;
    for removed in ["target/generated-man", "OUT_DIR", "clap_mangen"] {
        ensure!(
            !workflow.contains(removed),
            "build workflow should not reference {removed}"
        );
        ensure!(
            !staging.contains(removed),
            "release staging should not reference {removed}"
        );
    }
    Ok(())
}
