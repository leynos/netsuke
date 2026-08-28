//! Exercise release admission against isolated downstream GitHub API adapters.

#![cfg(unix)]

use std::{
    path::PathBuf,
    process::{Command, Output},
};

use anyhow::{Context, Result, ensure};
use proptest::prelude::*;
use serde_json::{Value, json};
use tempfile::TempDir;
use test_support::{fs as test_fs, write_exec_with_content};

const CANDIDATE_REVISION: &str = "a1b2c3d4";
const TEST_PATH: &str = "/usr/bin:/bin";
const WORKFLOW_PATH: &str = ".github/workflows/netsuke-canary.yml";
const WORKFLOW_BRANCH: &str = "issue-598-v010-netsuke-canary";
const MISSING_EVIDENCE: &str =
    "Missing successful Netsuke v0.1.0 release-admission canary candidate a1b2c3d4";
const CANARIES: [(&str, &str, u64); 3] = [
    (
        "leynos/repovec-appliance",
        "6be365b4b30ef48537add5719a9b387ccc41777f",
        343_316_513,
    ),
    (
        "leynos/mxd",
        "8146278cc82506c222bb78d4f3fc05c12ed95b41",
        343_314_513,
    ),
    (
        "leynos/ortho-config",
        "b42b5d0adfacd79456d2a2f9edbf9f561aac943b",
        343_328_370,
    ),
];
const MATCHING_WORKFLOW_SOURCE: &str = concat!(
    "am9iczoKICBjYW5hcnk6CiAgICBzdGVwczoKICAgICAgLSB1c2VzOiBsZXlub3MvbmV0c3Vr",
    "ZS8uZ2l0aHViL2FjdGlvbnMvaW5zdGFsbC1yZWxlYXNlLWNhbmRpZGF0ZUBhMWIyYzNkNAog",
    "ICAgICAgIHdpdGg6CiAgICAgICAgICByZXZpc2lvbjogYTFiMmMzZDQK"
);
const MISMATCHING_WORKFLOW_SOURCE: &str = concat!(
    "am9iczoKICBjYW5hcnk6CiAgICBzdGVwczoKICAgICAgLSB1c2VzOiBsZXlub3MvbmV0c3Vr",
    "ZS8uZ2l0aHViL2FjdGlvbnMvaW5zdGFsbC1yZWxlYXNlLWNhbmRpZGF0ZUBhMWIyYzNkNAog",
    "ICAgICAgIHdpdGg6CiAgICAgICAgICByZXZpc2lvbjogb3RoZXIK"
);
const COMMENT_ONLY_WORKFLOW_SOURCE: &str = concat!(
    "am9iczoKICBjYW5hcnk6CiAgICBzdGVwczoKICAgICAgIyB1c2VzOiBsZXlub3MvbmV0c3Vr",
    "ZS8uZ2l0aHViL2FjdGlvbnMvaW5zdGFsbC1yZWxlYXNlLWNhbmRpZGF0ZUBhMWIyYzNkNAog",
    "ICAgICAgIyByZXZpc2lvbjogYTFiMmMzZDQKICAgICAgLSBydW46IHRydWUK"
);
const SPLIT_STEP_WORKFLOW_SOURCE: &str = concat!(
    "am9iczoKICBjYW5hcnk6CiAgICBzdGVwczoKICAgICAgLSB1c2VzOiBsZXlub3MvbmV0c3Vr",
    "ZS8uZ2l0aHViL2FjdGlvbnMvaW5zdGFsbC1yZWxlYXNlLWNhbmRpZGF0ZUBhMWIyYzNkNAog",
    "ICAgICAgLSB3aXRoOgogICAgICAgICAgcmV2aXNpb246IGExYjJjM2Q0CiAgICAgICAgcnVu",
    "OiB0cnVlCg=="
);

#[derive(Clone, Copy, Debug)]
enum TrustField {
    Repository,
    WorkflowId,
    WorkflowPath,
    Event,
    Branch,
    DownstreamRevision,
    CandidateName,
    Status,
    Conclusion,
}

impl TrustField {
    const ALL: [Self; 9] = [
        Self::Repository,
        Self::WorkflowId,
        Self::WorkflowPath,
        Self::Event,
        Self::Branch,
        Self::DownstreamRevision,
        Self::CandidateName,
        Self::Status,
        Self::Conclusion,
    ];

    /// Change one field that the admission script requires from run evidence.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixture lacks the object required by the
    /// selected trust field.
    fn alter(self, run: &mut Value, variant: u8) -> Result<()> {
        let replacement = match self {
            Self::Repository => json!(format!("untrusted/repository-{variant}")),
            Self::WorkflowId => json!(900_000_u64 + u64::from(variant)),
            Self::WorkflowPath => json!(format!(".github/workflows/other-{variant}.yml")),
            Self::Event => json!(format!("workflow_dispatch_{variant}")),
            Self::Branch => json!(format!("untrusted-branch-{variant}")),
            Self::DownstreamRevision => json!(format!("untrusted-revision-{variant}")),
            Self::CandidateName => json!(format!("untrusted candidate {variant}")),
            Self::Status => json!(format!("queued-{variant}")),
            Self::Conclusion => json!(format!("failure-{variant}")),
        };
        let field = match self {
            Self::Repository => "repository",
            Self::WorkflowId => "workflow_id",
            Self::WorkflowPath => "path",
            Self::Event => "event",
            Self::Branch => "head_branch",
            Self::DownstreamRevision => "head_sha",
            Self::CandidateName => "name",
            Self::Status => "status",
            Self::Conclusion => "conclusion",
        };

        if let Self::Repository = self {
            let repository = run
                .get_mut(field)
                .and_then(Value::as_object_mut)
                .context("trusted fixture should contain a repository object")?;
            repository.insert("full_name".to_owned(), replacement);
        } else {
            let workflow_run = run
                .as_object_mut()
                .context("trusted fixture should contain a workflow-run object")?;
            workflow_run.insert(field.to_owned(), replacement);
        }

        Ok(())
    }
}

struct AdmissionHarness {
    root: TempDir,
    bash_env_path: PathBuf,
    fake_bin_dir: PathBuf,
    gh_args_path: PathBuf,
}

impl AdmissionHarness {
    /// Create an isolated environment containing a behavioural `gh` adapter.
    fn new() -> Result<Self> {
        let root = TempDir::new().context("create admission test directory")?;
        let fake_bin_dir = root.path().join("fake-bin");
        test_fs::create_dir(&fake_bin_dir).context("create fake command directory")?;
        let bash_env_path = root.path().join("bash-env");
        test_fs::write(&bash_env_path, "").context("write empty Bash environment")?;
        let gh_args_path = root.path().join("gh-args");
        write_exec_with_content(&fake_bin_dir, "gh", fake_gh_script())?;

        Ok(Self {
            root,
            bash_env_path,
            fake_bin_dir,
            gh_args_path,
        })
    }

    /// Run the production admission script against one workflow and run fixture.
    fn run(&self, workflow_source: &str, workflow_runs: &str) -> Result<Output> {
        Command::new("bash")
            .arg(admission_script())
            .env("BASH_ENV", &self.bash_env_path)
            .env("GITHUB_SHA", CANDIDATE_REVISION)
            .env("NETSUKE_GH_ARGS", &self.gh_args_path)
            .env("NETSUKE_WORKFLOW_SOURCE", workflow_source)
            .env("NETSUKE_WORKFLOW_RUNS", workflow_runs)
            .env(
                "PATH",
                format!("{}:{TEST_PATH}", self.fake_bin_dir.display()),
            )
            .env("RUNNER_TEMP", self.root.path())
            .output()
            .context("run release-admission canary script")
    }

    /// Read every complete `gh api` argument vector issued by the script.
    fn gh_args(&self) -> Result<String> {
        test_fs::read_to_string(&self.gh_args_path).context("read recorded GitHub API arguments")
    }
}

/// Locate the production admission script exercised by this test module.
fn admission_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".github/scripts/require-release-admission-canaries.sh")
}

/// Return the candidate-specific workflow name required by the shell script.
fn candidate_workflow_name() -> String {
    format!("Netsuke v0.1.0 release-admission canary candidate {CANDIDATE_REVISION}")
}

/// Build JSON workflow-run evidence that satisfies every production trust field.
fn trusted_workflow_runs() -> Result<String> {
    let workflow_name = candidate_workflow_name();
    let workflow_runs = CANARIES
        .iter()
        .enumerate()
        .map(|(index, (repository, revision, workflow_id))| {
            json!({
                "id": 9_001_u64 + index as u64,
                "repository": { "full_name": repository },
                "workflow_id": workflow_id,
                "path": WORKFLOW_PATH,
                "event": "push",
                "head_branch": WORKFLOW_BRANCH,
                "head_sha": revision,
                "name": workflow_name,
                "status": "completed",
                "conclusion": "success",
            })
        })
        .collect::<Vec<_>>();

    serde_json::to_string(&json!({ "workflow_runs": workflow_runs }))
        .context("serialize trusted workflow-run fixture")
}

/// Build evidence that differs from trusted evidence in exactly one field.
fn workflow_runs_with_mismatch(field: TrustField, variant: u8) -> Result<String> {
    let mut fixture: Value = serde_json::from_str(&trusted_workflow_runs()?)
        .context("parse trusted workflow-run fixture")?;
    let run = fixture
        .get_mut("workflow_runs")
        .and_then(Value::as_array_mut)
        .and_then(|runs| runs.first_mut())
        .context("trusted fixture should contain a workflow run")?;
    field.alter(run, variant)?;

    serde_json::to_string(&fixture).context("serialize mismatched workflow-run fixture")
}

/// Return the shell adapter that records and evaluates each production `gh api` call.
const fn fake_gh_script() -> &'static str {
    r#"#!/usr/bin/env bash
set -euo pipefail

if [[ "$1" != "api" ]]; then
  echo "unexpected gh invocation: $*" >&2
  exit 1
fi

printf '%q ' "$@" >> "${NETSUKE_GH_ARGS}"
printf '\n' >> "${NETSUKE_GH_ARGS}"

endpoint="$2"
jq_filter=""
while (($#)); do
  if [[ "$1" == "--jq" ]]; then
    jq_filter="$2"
    break
  fi
  shift
done

if [[ -z "$jq_filter" ]]; then
  echo "missing gh --jq filter" >&2
  exit 1
fi

if [[ "$endpoint" == *"/contents/"* ]]; then
  printf '{"content":"%s"}\n' "${NETSUKE_WORKFLOW_SOURCE}" | jq -r "$jq_filter"
  exit 0
fi

if [[ "$endpoint" == *"/actions/workflows/"*"/runs?"* ]]; then
  printf '%s\n' "${NETSUKE_WORKFLOW_RUNS}" | jq -r "$jq_filter"
  exit 0
fi

echo "unexpected gh endpoint: ${endpoint}" >&2
exit 1
"#
}

/// Assert that every complete `gh api` call reaches the expected endpoints.
fn require_recorded_api_arguments(harness: &AdmissionHarness) -> Result<()> {
    let gh_args = harness.gh_args()?;
    for (repository, revision, workflow_id) in CANARIES {
        ensure!(
            gh_args.contains(&format!(
                "repos/{repository}/contents/.github/workflows/netsuke-canary.yml\\?ref={revision}"
            )),
            "admission should fetch {repository}'s pinned workflow source"
        );
        ensure!(
            gh_args.contains(&format!(
                "actions/workflows/{workflow_id}/runs\\?head_sha={revision}\\&per_page=100"
            )),
            "admission should query {repository}'s pinned workflow runs"
        );
    }
    ensure!(
        gh_args.lines().count() == CANARIES.len() * 2,
        "admission should record every complete gh api argument vector"
    );

    Ok(())
}

/// Assert that a rejected fixture reports the production missing-evidence error.
fn require_missing_successful_evidence(output: &Output) -> Result<()> {
    ensure!(
        !output.status.success(),
        "admission should reject untrusted workflow-run evidence"
    );
    ensure!(
        String::from_utf8_lossy(&output.stderr).contains(MISSING_EVIDENCE),
        "admission should report missing successful candidate evidence"
    );

    Ok(())
}

/// Accept trusted evidence for all pinned canaries.
#[test]
fn admission_accepts_every_trusted_pinned_canary() -> Result<()> {
    let harness = AdmissionHarness::new()?;

    let output = harness.run(MATCHING_WORKFLOW_SOURCE, &trusted_workflow_runs()?)?;

    ensure!(
        output.status.success(),
        "admission should accept trusted evidence: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        stdout.matches("Accepted leynos/").count() == 3,
        "admission should accept every pinned canary"
    );
    require_recorded_api_arguments(&harness)
}

/// Reject evidence from a pinned workflow that did not test the candidate.
#[test]
fn admission_rejects_a_pinned_workflow_that_did_not_test_the_candidate() -> Result<()> {
    let harness = AdmissionHarness::new()?;

    let output = harness.run(MISMATCHING_WORKFLOW_SOURCE, &trusted_workflow_runs()?)?;

    ensure!(
        !output.status.success(),
        "admission should reject mismatched evidence"
    );
    ensure!(
        String::from_utf8_lossy(&output.stderr).contains("does not test a1b2c3d4"),
        "admission should identify the candidate mismatch"
    );
    ensure!(
        !harness.gh_args()?.contains("/actions/workflows/"),
        "admission should reject mismatched workflow source before checking runs"
    );

    Ok(())
}

/// Reject candidate references that appear only in comments or split steps.
#[rstest::rstest]
#[case(COMMENT_ONLY_WORKFLOW_SOURCE, "comment-only")]
#[case(SPLIT_STEP_WORKFLOW_SOURCE, "split-step")]
fn admission_rejects_non_executable_or_split_candidate_references(
    #[case] workflow_source: &str,
    #[case] fixture_name: &str,
) -> Result<()> {
    let harness = AdmissionHarness::new()?;

    let output = harness.run(workflow_source, &trusted_workflow_runs()?)?;

    ensure!(
        !output.status.success(),
        "admission should reject the {fixture_name} fixture"
    );
    ensure!(
        String::from_utf8_lossy(&output.stderr).contains("does not test a1b2c3d4"),
        "admission should identify the {fixture_name} candidate mismatch"
    );
    ensure!(
        !harness.gh_args()?.contains("/actions/workflows/"),
        "admission should reject the {fixture_name} fixture before checking runs"
    );

    Ok(())
}

/// Reject evidence when any trusted workflow-run field differs.
#[rstest::rstest]
#[case::repository(TrustField::Repository)]
#[case::workflow_id(TrustField::WorkflowId)]
#[case::workflow_path(TrustField::WorkflowPath)]
#[case::event(TrustField::Event)]
#[case::branch(TrustField::Branch)]
#[case::downstream_revision(TrustField::DownstreamRevision)]
#[case::candidate_name(TrustField::CandidateName)]
#[case::status(TrustField::Status)]
#[case::conclusion(TrustField::Conclusion)]
fn admission_rejects_each_altered_trust_field(#[case] field: TrustField) -> Result<()> {
    let harness = AdmissionHarness::new()?;
    let workflow_runs = workflow_runs_with_mismatch(field, 1)?;

    let output = harness.run(MATCHING_WORKFLOW_SOURCE, &workflow_runs)?;

    require_missing_successful_evidence(&output)
}

/// Reject admission when no successful trusted evidence is available.
#[test]
fn admission_rejects_missing_successful_evidence() -> Result<()> {
    let harness = AdmissionHarness::new()?;

    let output = harness.run(MATCHING_WORKFLOW_SOURCE, r#"{"workflow_runs":[]}"#)?;

    require_missing_successful_evidence(&output)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(8))]

    /// Accept only evidence whose independently varied trust fields all match.
    #[test]
    fn admission_requires_every_trust_field(variant in 0_u8..16) {
        let harness = AdmissionHarness::new().map_err(|error| TestCaseError::fail(error.to_string()))?;
        let trusted_runs = trusted_workflow_runs().map_err(|error| TestCaseError::fail(error.to_string()))?;
        let trusted = harness
            .run(MATCHING_WORKFLOW_SOURCE, &trusted_runs)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert!(
            trusted.status.success(),
            "admission should accept complete trusted evidence: {}",
            String::from_utf8_lossy(&trusted.stderr)
        );

        for field in TrustField::ALL {
            let altered_runs = workflow_runs_with_mismatch(field, variant)
                .map_err(|error| TestCaseError::fail(error.to_string()))?;
            let altered = harness
                .run(MATCHING_WORKFLOW_SOURCE, &altered_runs)
                .map_err(|error| TestCaseError::fail(error.to_string()))?;
            let stderr = String::from_utf8_lossy(&altered.stderr);
            prop_assert!(
                !altered.status.success(),
                "admission accepted altered {field:?} evidence"
            );
            prop_assert!(
                stderr.contains(MISSING_EVIDENCE),
                "admission should report missing evidence for altered {field:?}: {stderr}"
            );
        }
    }
}
