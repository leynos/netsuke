//! Validate build-and-package workflow wiring for shared actions.

mod common;

use anyhow::{Context, Result, ensure};
use common::workflow_contents;
use rstest::rstest;
use std::{fs, path::PathBuf};
use toml::Value;

fn release_staging_contents() -> Result<String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = root.join(".github").join("release-staging.toml");
    fs::read_to_string(&path)
        .with_context(|| format!("read release staging contents from {}", path.display()))
}

fn staging_config() -> Result<Value> {
    release_staging_contents()?
        .parse::<Value>()
        .context("parse release staging TOML")
}

fn artefact_sources(config: &Value) -> Result<Vec<&str>> {
    let artefacts = config
        .get("common")
        .and_then(|common| common.get("artefacts"))
        .and_then(Value::as_array)
        .context("common artefacts should be an array")?;
    artefacts
        .iter()
        .map(|artefact| {
            artefact
                .get("source")
                .and_then(Value::as_str)
                .context("artefact source should be a string")
        })
        .collect()
}

#[test]
fn behavioural_build_and_package_wiring_matches_shared_actions() {
    let contents = workflow_contents("build-and-package.yml")
        .expect("build-and-package workflow should be readable");

    assert!(
        contents.contains("stage-release-artefacts@"),
        "workflow should use shared stage-release-artefacts action"
    );
    assert!(
        contents.contains("normalize-windows-paths: ${{ inputs.platform == 'windows' }}"),
        "workflow should normalize Windows paths when staging on Windows"
    );
    assert!(
        contents.contains("application-path: ${{ steps.stage_paths.outputs.binary_path }}"),
        "windows-package should consume staged binary_path output"
    );
    assert!(
        contents.contains("license-rtf-path: ${{ steps.stage_paths.outputs.license_path }}"),
        "windows-package should consume staged license_path output"
    );
    assert!(
        contents.contains("upload-artefact: ${{ inputs['should-upload-workflow-artifacts'] }}"),
        "windows-package should use the upload-artefact input spelling"
    );
    assert!(
        contents.contains("binary: ${{ steps.stage_paths.outputs.binary_path }}"),
        "macos-package should consume staged binary_path output"
    );
    assert!(
        contents.contains("manpage: ${{ steps.stage_paths.outputs.man_path }}"),
        "macos-package should consume staged man_path output"
    );
    assert!(
        contents.contains("${{ steps.stage_paths.outputs.artifact_dir }}"),
        "workflow should use the staged artifact_dir output for uploads"
    );
}

#[test]
fn behavioural_build_and_package_generates_release_help_with_orthohelp() {
    let contents = workflow_contents("build-and-package.yml")
        .expect("build-and-package workflow should be readable");

    assert!(
        contents.contains("cargo install cargo-orthohelp --version 0.8.0 --locked"),
        "workflow should install the pinned cargo-orthohelp release tool"
    );
    assert!(
        contents.contains("scripts/generate-release-help.sh"),
        "workflow should call the release help script"
    );
    assert!(
        contents.contains("\"target/orthohelp/${{ inputs.target }}/release\""),
        "workflow should generate help under target/orthohelp"
    );
    assert!(
        contents.contains("man-paths: ${{ steps.stage_paths.outputs.man_path }}"),
        "Linux packaging should consume the staged man_path output"
    );
    assert!(
        !contents.contains("target/generated-man"),
        "workflow should not rely on build.rs generated man pages"
    );
}

#[rstest]
#[case("Stage artefacts")]
#[case("Capture staged paths")]
fn behavioural_staging_runs_for_every_platform(#[case] step_name: &str) {
    let contents = workflow_contents("build-and-package.yml")
        .expect("build-and-package workflow should be readable");
    let step = format!("- name: {step_name}");
    let step_body = contents
        .lines()
        .skip_while(|line| !line.contains(&step))
        .take_while(|line| !line.contains("      - name: ") || line.contains(&step))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        step_body.contains(&step),
        "{step_name} step should exist in the workflow"
    );
    assert!(
        !step_body.contains("if: inputs.platform != 'linux'"),
        "{step_name} should run for Linux as well as Windows and macOS"
    );
}

#[rstest]
#[case("target/orthohelp/{target}/release/man/man1/{bin_name}.1")]
#[case("target/orthohelp/{target}/release/powershell/Netsuke/Netsuke.psm1")]
#[case("target/orthohelp/{target}/release/powershell/Netsuke/Netsuke.psd1")]
#[case("target/orthohelp/{target}/release/powershell/Netsuke/en-US/Netsuke-help.xml")]
#[case("target/orthohelp/{target}/release/powershell/Netsuke/en-US/about_Netsuke.help.txt")]
fn release_staging_declares_orthohelp_outputs(#[case] expected_source: &str) -> Result<()> {
    let config = staging_config()?;
    let sources = artefact_sources(&config)?;
    ensure!(
        sources.contains(&expected_source),
        "expected release staging source {expected_source}, got {sources:?}"
    );
    Ok(())
}

#[rstest]
#[case("target/generated-man")]
#[case("OUT_DIR")]
#[case("clap_mangen")]
fn release_staging_does_not_reference_build_script_help_paths(
    #[case] removed_fragment: &str,
) -> Result<()> {
    let contents = release_staging_contents()?;
    ensure!(
        !contents.contains(removed_fragment),
        "release staging should not reference {removed_fragment}"
    );
    Ok(())
}

#[test]
fn orthohelp_man_page_has_no_out_dir_alternative() -> Result<()> {
    let config = staging_config()?;
    let artefacts = config
        .get("common")
        .and_then(|common| common.get("artefacts"))
        .and_then(Value::as_array)
        .context("common artefacts should be an array")?;
    let man_page = artefacts
        .iter()
        .find(|artefact| {
            artefact.get("source").and_then(Value::as_str)
                == Some("target/orthohelp/{target}/release/man/man1/{bin_name}.1")
        })
        .context("orthohelp man page artefact should be declared")?;

    ensure!(
        man_page.get("alternatives").is_none(),
        "orthohelp man page should not fall back to Cargo OUT_DIR"
    );
    Ok(())
}
