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
    let mut sources = Vec::new();
    let common_artefacts = config
        .get("common")
        .and_then(|common| common.get("artefacts"))
        .and_then(Value::as_array)
        .context("common artefacts should be an array")?;
    for artefact in common_artefacts {
        sources.push(
            artefact
                .get("source")
                .and_then(Value::as_str)
                .context("common artefact source should be a string")?,
        );
    }

    let targets = config
        .get("targets")
        .and_then(Value::as_table)
        .context("targets should be a table")?;
    for target in targets.values() {
        let Some(artefacts) = target.get("artefacts").and_then(Value::as_array) else {
            continue;
        };
        for artefact in artefacts {
            sources.push(
                artefact
                    .get("source")
                    .and_then(Value::as_str)
                    .context("target artefact source should be a string")?,
            );
        }
    }

    Ok(sources)
}

fn flush_block(current: &mut Vec<&str>, blocks: &mut Vec<String>) {
    if !current.is_empty() {
        let block = current.join("\n");
        if block.contains("rust-build-release@") {
            blocks.push(block);
        }
        current.clear();
    }
}

fn rust_build_release_step_blocks(contents: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current_block = Vec::new();

    for line in contents.lines() {
        if line.starts_with("      - ") {
            flush_block(&mut current_block, &mut blocks);
        }
        current_block.push(line);
    }

    flush_block(&mut current_block, &mut blocks);
    blocks
}

fn workflow_step_body<'a>(contents: &'a str, step_name: &str) -> Vec<&'a str> {
    let step = format!("- name: {step_name}");
    contents
        .lines()
        .skip_while(|line| !line.contains(&step))
        .take_while(|line| !line.contains("      - name: ") || line.contains(&step))
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
    let rust_build_steps = rust_build_release_step_blocks(&contents);
    assert!(
        !rust_build_steps.is_empty(),
        "workflow should call rust-build-release"
    );
    for step in rust_build_steps {
        assert!(
            step.contains("skip-man-page-discovery: 'true'"),
            "rust-build-release call should skip embedded man-page discovery"
        );
    }
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
#[case("config-file: .github/release-staging.toml")]
#[case("man-paths: ${{ steps.stage_paths.outputs.man_path }}")]
fn build_and_package_wires_staged_release_outputs(#[case] expected: &str) {
    let contents = workflow_contents("build-and-package.yml")
        .expect("build-and-package workflow should be readable");

    assert!(
        contents.contains(expected),
        "build-and-package workflow should contain {expected}"
    );
}

#[test]
fn windows_upload_includes_staged_artifact_dir() {
    let contents = workflow_contents("build-and-package.yml")
        .expect("build-and-package workflow should be readable");
    let step_body = workflow_step_body(&contents, "Upload Windows artefacts").join("\n");

    assert!(
        step_body.contains("${{ steps.stage_paths.outputs.artifact_dir }}"),
        "Windows upload should include staged sidecar artefacts"
    );
}

#[rstest]
#[case("Stage artefacts")]
#[case("Capture staged paths")]
fn behavioural_staging_runs_for_every_platform(#[case] step_name: &str) {
    let contents = workflow_contents("build-and-package.yml")
        .expect("build-and-package workflow should be readable");
    let step = format!("- name: {step_name}");
    let step_body = workflow_step_body(&contents, step_name).join("\n");

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
    let targets = config
        .get("targets")
        .and_then(Value::as_table)
        .context("targets should be a table")?;
    let man_page = targets
        .values()
        .filter_map(|target| target.get("artefacts").and_then(Value::as_array))
        .flatten()
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
