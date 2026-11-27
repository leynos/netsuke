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
    CANDIDATE_REPOSITORY, CANDIDATE_REVISION, CANDIDATE_VERSION, InstallerHarness, InstallerRun,
    binary_name, require_failure_event, require_successful_events,
};

// Verify that the composite action maps its installer interface.
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

// Verify that the installer emits the expected platform-specific candidate.
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
        harness
            .cargo_args()?
            .starts_with("build --locked --release --bin netsuke --target-dir "),
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

// Verify that malformed candidate revisions are rejected before Git runs.
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

// A reusable-workflow caller's candidate may exist only in the caller's
// repository, so the installer fetches from the repository it was resolved in.
#[rstest]
#[case::upstream(CANDIDATE_REPOSITORY)]
#[case::caller("example-owner/netsuke.fork")]
fn installer_fetches_from_the_resolved_repository(#[case] repository: &str) -> Result<()> {
    let harness = InstallerHarness::new()?;

    let output = harness.run_inheriting(&[("NETSUKE_CANDIDATE_REPOSITORY", repository)])?;

    ensure!(
        output.status.success(),
        "installer should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let git_args = harness.git_args()?;
    ensure!(
        git_args.contains(&format!(
            "remote add origin https://github.com/{repository}.git"
        )),
        "installer should fetch from {repository}, got: {git_args}"
    );
    require_successful_events(&output)?;

    Ok(())
}

// Verify that a repository that is not a GitHub `owner/name` never reaches Git.
#[rstest]
#[case::empty("")]
#[case::owner_only("leynos")]
#[case::option_owner("-c/netsuke")]
#[case::option("--upload-pack=x/y")]
#[case::nested("leynos/netsuke/extra")]
#[case::parent_name("leynos/..")]
#[case::current_name("leynos/.")]
#[case::embedded_newline("leynos/netsuke\n--upload-pack=x")]
#[case::non_ascii("l\u{e9}ynos/netsuke")]
fn installer_rejects_a_malformed_candidate_repository(#[case] repository: &str) -> Result<()> {
    let harness = InstallerHarness::new()?;

    let output = harness.run_inheriting(&[("NETSUKE_CANDIDATE_REPOSITORY", repository)])?;

    ensure!(
        !output.status.success(),
        "installer should reject the candidate repository {repository:?}"
    );
    ensure!(
        String::from_utf8_lossy(&output.stderr)
            .contains("candidate repository must be a GitHub owner/name"),
        "installer should explain the candidate repository format requirement"
    );
    ensure!(
        !harness.git_args_path.exists(),
        "installer should reject {repository:?} before running Git"
    );
    require_failure_event(
        &output,
        "candidate_repository_validation",
        "invalid_candidate_repository",
    )?;

    Ok(())
}

// Verify that invalid candidate identity stops or permits the build as expected.
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

// Verify that the candidate is built in the shipped release shape.
//
// Assigning `RUSTFLAGS`, even to an empty value, displaces the checkout's
// development `.cargo/config.toml` flags, including the Linux `mold` linker a
// hosted runner lacks, while a caller's own flags still reach Cargo.
#[rstest]
#[case::unset(None, "")]
#[case::inherited(Some("-C debuginfo=0"), "-C debuginfo=0")]
fn installer_builds_in_the_release_rustflags_shape(
    #[case] inherited: Option<&str>,
    #[case] expected: &str,
) -> Result<()> {
    let harness = InstallerHarness::new()?;

    let environment: Vec<_> = inherited
        .map(|flags| ("RUSTFLAGS", flags))
        .into_iter()
        .collect();
    let output = harness.run_inheriting(&environment)?;

    ensure!(
        output.status.success(),
        "installer should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        harness.cargo_rustflags()? == format!("{expected}\n"),
        "installer should assign RUSTFLAGS to the inherited value, got {:?}",
        harness.cargo_rustflags()?
    );

    Ok(())
}

// A downstream workflow may redirect Cargo's output for its own builds. The
// installer is public, so it must still find the candidate it just built.
#[rstest]
#[case::target_directory("CARGO_TARGET_DIR", "redirected-target")]
#[case::target_triple("CARGO_BUILD_TARGET", "x86_64-unknown-linux-gnu")]
fn installer_ignores_an_inherited_output_location(
    #[case] variable: &str,
    #[case] value: &str,
) -> Result<()> {
    let harness = InstallerHarness::new()?;

    let output = harness.run_inheriting(&[(variable, value)])?;

    ensure!(
        output.status.success(),
        "installer should find its build despite {variable}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        harness.outputs()?.contains_key("binary"),
        "installer should expose the binary despite {variable}"
    );

    Ok(())
}
