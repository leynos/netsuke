//! Exercise release admission against isolated downstream GitHub API adapters.

#![cfg(unix)]

use std::{
    path::PathBuf,
    process::{Command, Output},
};

use anyhow::{Context, Result, ensure};
use tempfile::TempDir;
use test_support::{fs as test_fs, write_exec_with_content};

const CANDIDATE_REVISION: &str = "a1b2c3d4";
const TEST_PATH: &str = "/usr/bin:/bin";
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

struct AdmissionHarness {
    root: TempDir,
    bash_env_path: PathBuf,
    fake_bin_dir: PathBuf,
    gh_args_path: PathBuf,
}

impl AdmissionHarness {
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

    fn run(&self, mode: &str, workflow_source: &str) -> Result<Output> {
        Command::new("bash")
            .arg(admission_script())
            .env("BASH_ENV", &self.bash_env_path)
            .env("GITHUB_SHA", CANDIDATE_REVISION)
            .env("NETSUKE_GH_ARGS", &self.gh_args_path)
            .env("NETSUKE_GH_MODE", mode)
            .env("NETSUKE_WORKFLOW_SOURCE", workflow_source)
            .env(
                "PATH",
                format!("{}:{TEST_PATH}", self.fake_bin_dir.display()),
            )
            .env("RUNNER_TEMP", self.root.path())
            .output()
            .context("run release-admission canary script")
    }

    fn gh_args(&self) -> Result<String> {
        test_fs::read_to_string(&self.gh_args_path).context("read recorded GitHub API arguments")
    }
}

fn admission_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".github/scripts/require-release-admission-canaries.sh")
}

const fn fake_gh_script() -> &'static str {
    r#"#!/usr/bin/env bash
set -euo pipefail

if [[ "$1" != "api" ]]; then
  echo "unexpected gh invocation: $*" >&2
  exit 1
fi

if [[ "$2" == *"/contents/"* ]]; then
  printf '%s\n' "${NETSUKE_WORKFLOW_SOURCE}"
  exit 0
fi

printf '%s\n' "$*" >> "${NETSUKE_GH_ARGS}"
if [[ "${NETSUKE_GH_MODE}" == "success" ]]; then
  printf '9001\n'
fi
"#
}

#[test]
fn admission_accepts_every_trusted_pinned_canary() -> Result<()> {
    let harness = AdmissionHarness::new()?;

    let output = harness.run("success", MATCHING_WORKFLOW_SOURCE)?;

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

    let gh_args = harness.gh_args()?;
    ensure!(gh_args.contains("actions/workflows/343316513/runs?head_sha=6be365b4b30ef48537add5719a9b387ccc41777f&per_page=100"));
    ensure!(gh_args.contains("actions/workflows/343314513/runs?head_sha=8146278cc82506c222bb78d4f3fc05c12ed95b41&per_page=100"));
    ensure!(gh_args.contains("actions/workflows/343328370/runs?head_sha=b42b5d0adfacd79456d2a2f9edbf9f561aac943b&per_page=100"));

    Ok(())
}

#[test]
fn admission_rejects_a_pinned_workflow_that_did_not_test_the_candidate() -> Result<()> {
    let harness = AdmissionHarness::new()?;

    let output = harness.run("success", MISMATCHING_WORKFLOW_SOURCE)?;

    ensure!(
        !output.status.success(),
        "admission should reject mismatched evidence"
    );
    ensure!(
        String::from_utf8_lossy(&output.stderr).contains("does not test a1b2c3d4"),
        "admission should identify the candidate mismatch"
    );
    ensure!(
        !harness.gh_args_path.exists(),
        "admission should reject mismatched workflow source before checking runs"
    );

    Ok(())
}

#[rstest::rstest]
#[case(COMMENT_ONLY_WORKFLOW_SOURCE, "comment-only")]
#[case(SPLIT_STEP_WORKFLOW_SOURCE, "split-step")]
fn admission_rejects_non_executable_or_split_candidate_references(
    #[case] workflow_source: &str,
    #[case] fixture_name: &str,
) -> Result<()> {
    let harness = AdmissionHarness::new()?;

    let output = harness.run("success", workflow_source)?;

    ensure!(
        !output.status.success(),
        "admission should reject the {fixture_name} fixture"
    );
    ensure!(
        String::from_utf8_lossy(&output.stderr).contains("does not test a1b2c3d4"),
        "admission should identify the {fixture_name} candidate mismatch"
    );
    ensure!(
        !harness.gh_args_path.exists(),
        "admission should reject the {fixture_name} fixture before checking runs"
    );

    Ok(())
}

#[test]
fn admission_rejects_missing_successful_evidence() -> Result<()> {
    let harness = AdmissionHarness::new()?;

    let output = harness.run("missing", MATCHING_WORKFLOW_SOURCE)?;

    ensure!(
        !output.status.success(),
        "admission should reject a missing run"
    );
    ensure!(
        String::from_utf8_lossy(&output.stderr).contains(
            "Missing successful Netsuke v0.1.0 release-admission canary candidate a1b2c3d4"
        ),
        "admission should identify the missing candidate evidence"
    );

    Ok(())
}
