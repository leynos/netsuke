//! Exercise the release-candidate installer against isolated command adapters.

#![cfg(unix)]

#[path = "release_candidate_installer/action_contract.rs"]
mod action_contract;
#[path = "release_candidate_installer/harness.rs"]
mod harness;

use anyhow::{Context, Result, ensure};
use rstest::rstest;

use action_contract::{
    release_candidate_action, require_candidate_inputs, require_candidate_installer_step,
    require_candidate_outputs,
};
use harness::{
    CANDIDATE_REVISION, CANDIDATE_VERSION, InstallerHarness, InstallerRun, binary_name,
    require_failure_event, require_successful_events,
};

/// Verify that the composite action maps its installer interface.
#[test]
fn composite_action_maps_the_candidate_installer_interface() -> Result<()> {
    let action = release_candidate_action()?;
    let root = action
        .as_mapping()
        .context("release-candidate composite action should be a mapping")?;

    require_candidate_inputs(root)?;
    require_candidate_installer_step(root)?;
    require_candidate_outputs(root)?;

    Ok(())
}

/// Verify that the installer emits the expected platform-specific candidate.
#[rstest]
#[case("Linux")]
#[case("Windows")]
fn installer_builds_the_expected_platform_binary(#[case] runner_os: &str) -> Result<()> {
    let harness = InstallerHarness::new()?;

    let output = harness.run(runner_os, CANDIDATE_REVISION, CANDIDATE_VERSION)?;

    ensure!(
        output.status.success(),
        "installer should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        harness.cargo_args()? == "build --locked --release --bin netsuke\n",
        "installer should build a locked release candidate"
    );
    ensure!(
        harness
            .git_args()?
            .contains(&format!("fetch --depth 1 origin -- {CANDIDATE_REVISION}")),
        "installer should fetch the validated candidate revision after an option terminator"
    );
    require_successful_events(&output)?;

    let outputs = harness.outputs()?;
    ensure!(
        outputs.get("revision") == Some(&CANDIDATE_REVISION.to_owned()),
        "installer should expose the verified revision"
    );
    ensure!(
        outputs.get("version") == Some(&CANDIDATE_VERSION.to_owned()),
        "installer should expose the verified version"
    );
    ensure!(
        outputs
            .get("binary")
            .is_some_and(|path| path.ends_with(binary_name(runner_os))),
        "installer should expose the {runner_os} binary name"
    );

    Ok(())
}

/// Verify that malformed candidate revisions are rejected before Git runs.
#[rstest]
#[case("short-revision")]
#[case("--upload-pack=/tmp/untrusted")]
fn installer_rejects_non_commit_candidate_revisions(
    #[case] candidate_revision: &str,
) -> Result<()> {
    let harness = InstallerHarness::new()?;

    let output = harness.run_with_candidate_revision(&InstallerRun {
        runner_os: "Linux",
        candidate_revision,
        resolved_revision: CANDIDATE_REVISION,
        version: CANDIDATE_VERSION,
    })?;

    ensure!(
        !output.status.success(),
        "installer should reject a non-commit candidate revision"
    );
    ensure!(
        String::from_utf8_lossy(&output.stderr)
            .contains("candidate revision must be a full 40-character hexadecimal commit"),
        "installer should explain the candidate revision format requirement"
    );
    ensure!(
        !harness.git_args_path.exists(),
        "installer should reject a malformed candidate revision before running Git"
    );
    require_failure_event(
        &output,
        "candidate_revision_validation",
        "invalid_candidate_revision",
    )?;

    Ok(())
}

/// Verify that invalid candidate identity stops or permits the build as expected.
#[rstest]
#[case::revision_mismatch(
    "different-revision",
    CANDIDATE_VERSION,
    "candidate revision mismatch",
    false
)]
#[case::version_mismatch(CANDIDATE_REVISION, "0.1.0-wrong", "candidate version mismatch", true)]
fn installer_rejects_an_invalid_candidate_identity(
    #[case] resolved_revision: &str,
    #[case] version: &str,
    #[case] expected_stderr: &str,
    #[case] should_build_cargo: bool,
) -> Result<()> {
    let harness = InstallerHarness::new()?;

    let output = harness.run("Linux", resolved_revision, version)?;

    ensure!(
        !output.status.success(),
        "installer should reject an invalid candidate identity"
    );
    ensure!(
        String::from_utf8_lossy(&output.stderr).contains(expected_stderr),
        "installer should report the expected identity validation error"
    );
    ensure!(
        harness.cargo_args_path.exists() == should_build_cargo,
        "installer Cargo build should match the candidate identity failure"
    );
    let (operation, error_category) = if should_build_cargo {
        ("candidate_version_check", "candidate_version_mismatch")
    } else {
        ("candidate_revision_verification", "revision_mismatch")
    };
    require_failure_event(&output, operation, error_category)?;

    Ok(())
}
