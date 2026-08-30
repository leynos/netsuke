//! Exercise the release-candidate installer against isolated command adapters.

#![cfg(unix)]

use std::{
    collections::BTreeMap,
    path::PathBuf,
    process::{Command, Output},
};

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use serde_yaml::{Mapping, Value as YamlValue};
use tempfile::TempDir;
use test_support::{fs as test_fs, write_exec_with_content};

const CANDIDATE_REVISION: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678";
const CANDIDATE_VERSION: &str = "0.1.0-beta2";
const TEST_PATH: &str = "/usr/bin:/bin";
const TEST_TOKEN: &str = "installer-test-token";
const INSTALLER_OPERATIONS: [&str; 5] = [
    "candidate_revision_validation",
    "git_fetch",
    "candidate_revision_verification",
    "locked_cargo_build",
    "candidate_version_check",
];

/// Represent controlled installer inputs for one harness invocation.
struct InstallerRun<'a> {
    runner_os: &'a str,
    candidate_revision: &'a str,
    resolved_revision: &'a str,
    version: &'a str,
}

struct InstallerHarness {
    root: TempDir,
    bash_env_path: PathBuf,
    fake_bin_dir: PathBuf,
    git_args_path: PathBuf,
    cargo_args_path: PathBuf,
    github_output_path: PathBuf,
}

impl InstallerHarness {
    /// Create an isolated installer harness.
    fn new() -> Result<Self> {
        let root = TempDir::new().context("create installer test directory")?;
        let fake_bin_dir = root.path().join("fake-bin");
        test_fs::create_dir(&fake_bin_dir).context("create fake command directory")?;
        let cargo_args_path = root.path().join("cargo-args");
        let git_args_path = root.path().join("git-args");
        let github_output_path = root.path().join("github-output");
        let bash_env_path = root.path().join("bash-env");
        test_fs::write(&bash_env_path, "").context("write empty Bash environment")?;

        write_exec_with_content(&fake_bin_dir, "git", fake_git_script())?;
        write_exec_with_content(&fake_bin_dir, "cargo", fake_cargo_script())?;

        Ok(Self {
            root,
            bash_env_path,
            fake_bin_dir,
            git_args_path,
            cargo_args_path,
            github_output_path,
        })
    }

    /// Run the installer with controlled identity and platform inputs.
    fn run(&self, runner_os: &str, resolved_revision: &str, version: &str) -> Result<Output> {
        self.run_with_candidate_revision(&InstallerRun {
            runner_os,
            candidate_revision: CANDIDATE_REVISION,
            resolved_revision,
            version,
        })
    }

    /// Run the installer with a caller-supplied candidate revision.
    fn run_with_candidate_revision(&self, inputs: &InstallerRun<'_>) -> Result<Output> {
        Command::new("bash")
            .arg(installer_script())
            .env("BASH_ENV", &self.bash_env_path)
            .env("GITHUB_OUTPUT", &self.github_output_path)
            .env("NETSUKE_CANDIDATE_REVISION", inputs.candidate_revision)
            .env("NETSUKE_CANDIDATE_VERSION", CANDIDATE_VERSION)
            .env("NETSUKE_CARGO_ARGS", &self.cargo_args_path)
            .env("NETSUKE_GIT_ARGS", &self.git_args_path)
            .env("GH_TOKEN", TEST_TOKEN)
            .env("NETSUKE_FAKE_BINARY_NAME", binary_name(inputs.runner_os))
            .env("NETSUKE_FAKE_RESOLVED_REVISION", inputs.resolved_revision)
            .env("NETSUKE_FAKE_VERSION", inputs.version)
            .env(
                "PATH",
                format!("{}:{TEST_PATH}", self.fake_bin_dir.display()),
            )
            .env("RUNNER_OS", inputs.runner_os)
            .env("RUNNER_TEMP", self.root.path())
            .output()
            .context("run release-candidate installer")
    }

    /// Read installer outputs emitted through `GITHUB_OUTPUT`.
    fn outputs(&self) -> Result<BTreeMap<String, String>> {
        let contents = test_fs::read_to_string(&self.github_output_path)
            .context("read installer GitHub output")?;
        Ok(contents
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect())
    }

    /// Read recorded Cargo build arguments.
    fn cargo_args(&self) -> Result<String> {
        test_fs::read_to_string(&self.cargo_args_path).context("read recorded cargo arguments")
    }

    /// Read recorded Git command arguments.
    fn git_args(&self) -> Result<String> {
        test_fs::read_to_string(&self.git_args_path).context("read recorded Git arguments")
    }
}

/// Locate the production installer script.
fn installer_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".github/actions/install-release-candidate/install.sh")
}

/// Return the platform-specific candidate binary name.
fn binary_name(runner_os: &str) -> &'static str {
    if runner_os == "Windows" {
        "netsuke.exe"
    } else {
        "netsuke"
    }
}

/// Require successful installer events to cover every bounded operation.
fn require_successful_events(output: &Output) -> Result<()> {
    let stderr = String::from_utf8_lossy(&output.stderr);
    for operation in INSTALLER_OPERATIONS {
        ensure!(
            stderr.contains(&format!(
                "release_candidate operation={operation} outcome=started"
            )) && stderr.contains(&format!(
                "release_candidate operation={operation} outcome=success"
            )),
            "installer should emit started and success events for {operation}"
        );
    }
    ensure!(
        !stderr.contains(TEST_TOKEN),
        "installer events should not expose the test token"
    );

    Ok(())
}

/// Require a controlled installer failure to retain its fixed event category.
fn require_failure_event(output: &Output, operation: &str, error_category: &str) -> Result<()> {
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        stderr.contains(&format!(
            "release_candidate operation={operation} outcome=failure error_category={error_category}"
        )),
        "installer should emit a fixed failure event for {operation}"
    );
    ensure!(
        !stderr.contains(TEST_TOKEN),
        "installer events should not expose the test token"
    );

    Ok(())
}

/// Return the YAML value stored under `key` in one mapping.
fn mapping_value<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a YamlValue> {
    mapping.get(YamlValue::String(key.to_owned()))
}

/// Return the fake Git adapter script.
const fn fake_git_script() -> &'static str {
    r#"#!/usr/bin/env bash
set -euo pipefail

if [[ "$1" == "-C" ]]; then
  shift 2
fi

printf '%q ' "$@" >> "${NETSUKE_GIT_ARGS}"
printf '\n' >> "${NETSUKE_GIT_ARGS}"

case "$1" in
  init|remote|fetch|checkout) exit 0 ;;
  rev-parse) printf '%s\n' "${NETSUKE_FAKE_RESOLVED_REVISION}" ;;
  *) echo "unexpected git invocation: $*" >&2; exit 1 ;;
esac
"#
}

/// Return the fake Cargo adapter script.
const fn fake_cargo_script() -> &'static str {
    r#"#!/usr/bin/env bash
set -euo pipefail

if [[ "$*" != "build --locked --release --bin netsuke" ]]; then
  echo "unexpected cargo invocation: $*" >&2
  exit 1
fi

printf '%s\n' "$*" > "${NETSUKE_CARGO_ARGS}"
mkdir -p target/release
printf '#!/usr/bin/env bash\nprintf "netsuke %%s\\n" "${NETSUKE_FAKE_VERSION}"\n' \
  > "target/release/${NETSUKE_FAKE_BINARY_NAME}"
chmod +x "target/release/${NETSUKE_FAKE_BINARY_NAME}"
"#
}

/// Parse the release-candidate composite action.
fn release_candidate_action() -> Result<YamlValue> {
    let contents = test_fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".github/actions/install-release-candidate/action.yml"),
    )
    .context("read release-candidate composite action")?;

    serde_yaml::from_str(&contents).context("parse release-candidate composite action")
}

/// Require the composite action to expose both required candidate inputs.
fn require_candidate_inputs(root: &Mapping) -> Result<()> {
    let inputs = mapping_value(root, "inputs")
        .and_then(YamlValue::as_mapping)
        .context("release-candidate composite action should declare inputs")?;

    ensure!(
        inputs.len() == 2
            && mapping_value(inputs, "revision")
                .and_then(YamlValue::as_mapping)
                .and_then(|input| mapping_value(input, "required"))
                .and_then(YamlValue::as_bool)
                == Some(true)
            && mapping_value(inputs, "expected-version")
                .and_then(YamlValue::as_mapping)
                .and_then(|input| mapping_value(input, "required"))
                .and_then(YamlValue::as_bool)
                == Some(true),
        "the composite action should require its revision and expected-version inputs"
    );

    Ok(())
}

/// Require the composite action to map its inputs into the installer step.
fn require_candidate_installer_step(root: &Mapping) -> Result<()> {
    let runs = mapping_value(root, "runs")
        .and_then(YamlValue::as_mapping)
        .context("release-candidate composite action should declare runs")?;
    let steps = mapping_value(runs, "steps")
        .and_then(YamlValue::as_sequence)
        .context("release-candidate composite action should declare steps")?;
    let install_step = steps
        .first()
        .and_then(YamlValue::as_mapping)
        .context("release-candidate composite action should declare an install step")?;
    let environment = mapping_value(install_step, "env")
        .and_then(YamlValue::as_mapping)
        .context("release-candidate install step should declare an environment")?;

    ensure!(
        mapping_value(install_step, "id").and_then(YamlValue::as_str) == Some("install")
            && mapping_value(install_step, "run").and_then(YamlValue::as_str)
                == Some("bash \"${GITHUB_ACTION_PATH}/install.sh\""),
        "the composite action should invoke the installer covered by this harness"
    );
    ensure!(
        environment.len() == 2
            && mapping_value(environment, "NETSUKE_CANDIDATE_REVISION").and_then(YamlValue::as_str)
                == Some("${{ inputs.revision }}")
            && mapping_value(environment, "NETSUKE_CANDIDATE_VERSION").and_then(YamlValue::as_str)
                == Some("${{ inputs.expected-version }}"),
        "the composite action should map both candidate inputs into the installer environment"
    );

    Ok(())
}

/// Require the composite action to expose every verified installer output.
fn require_candidate_outputs(root: &Mapping) -> Result<()> {
    let outputs = mapping_value(root, "outputs")
        .and_then(YamlValue::as_mapping)
        .context("release-candidate composite action should declare outputs")?;

    ensure!(
        outputs.len() == 3
            && mapping_value(outputs, "binary")
                .and_then(YamlValue::as_mapping)
                .and_then(|output| mapping_value(output, "value"))
                .and_then(YamlValue::as_str)
                == Some("${{ steps.install.outputs.binary }}")
            && mapping_value(outputs, "revision")
                .and_then(YamlValue::as_mapping)
                .and_then(|output| mapping_value(output, "value"))
                .and_then(YamlValue::as_str)
                == Some("${{ steps.install.outputs.revision }}")
            && mapping_value(outputs, "version")
                .and_then(YamlValue::as_mapping)
                .and_then(|output| mapping_value(output, "value"))
                .and_then(YamlValue::as_str)
                == Some("${{ steps.install.outputs.version }}"),
        "the composite action should expose every verified installer output"
    );

    Ok(())
}

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
