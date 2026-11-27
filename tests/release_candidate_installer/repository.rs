//! Exercise the installer's choice of the repository it fetches from.
//!
//! A reusable-workflow caller's candidate may exist only in the caller's
//! repository, so the installer fetches from the repository the candidate was
//! resolved in, and refuses any value that is not a GitHub `owner/name` before
//! Git runs.

use anyhow::{Result, ensure};
use rstest::rstest;

use super::harness::{
    CANDIDATE_REPOSITORY, CandidateRepository, InstallerHarness, require_failure_event,
    require_successful_events,
};

// A reusable-workflow caller's candidate may exist only in the caller's
// repository, so the installer fetches from the repository it was resolved in.
#[rstest]
#[case::upstream(CandidateRepository(CANDIDATE_REPOSITORY))]
#[case::caller(CandidateRepository("example-owner/netsuke.fork"))]
fn installer_fetches_from_the_resolved_repository(
    #[case] repository: CandidateRepository,
) -> Result<()> {
    let harness = InstallerHarness::new()?;

    let output = harness.run_from(repository)?;
    let CandidateRepository(name) = repository;

    ensure!(
        output.status.success(),
        "installer should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let git_args = harness.git_args()?;
    ensure!(
        git_args.contains(&format!("remote add origin https://github.com/{name}.git")),
        "installer should fetch from {name}, got: {git_args}"
    );
    require_successful_events(&output)?;

    Ok(())
}

// Verify that a repository that is not a GitHub `owner/name` never reaches Git.
#[rstest]
#[case::empty(CandidateRepository(""))]
#[case::owner_only(CandidateRepository("leynos"))]
#[case::option_owner(CandidateRepository("-c/netsuke"))]
#[case::option(CandidateRepository("--upload-pack=x/y"))]
#[case::nested(CandidateRepository("leynos/netsuke/extra"))]
#[case::parent_name(CandidateRepository("leynos/.."))]
#[case::current_name(CandidateRepository("leynos/."))]
#[case::embedded_newline(CandidateRepository("leynos/netsuke\n--upload-pack=x"))]
#[case::non_ascii(CandidateRepository("l\u{e9}ynos/netsuke"))]
fn installer_rejects_a_malformed_candidate_repository(
    #[case] repository: CandidateRepository,
) -> Result<()> {
    let harness = InstallerHarness::new()?;

    let output = harness.run_from(repository)?;
    let CandidateRepository(name) = repository;

    ensure!(
        !output.status.success(),
        "installer should reject the candidate repository {name:?}"
    );
    ensure!(
        String::from_utf8_lossy(&output.stderr)
            .contains("candidate repository must be a GitHub owner/name"),
        "installer should explain the candidate repository format requirement"
    );
    ensure!(
        !harness.git_args_path.exists(),
        "installer should reject {name:?} before running Git"
    );
    require_failure_event(
        &output,
        "candidate_repository_validation",
        "invalid_candidate_repository",
    )?;

    Ok(())
}
