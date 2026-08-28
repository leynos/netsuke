#![cfg(unix)]

//! Exercise the release-candidate installer against isolated command adapters.

use std::{
    collections::BTreeMap,
    path::PathBuf,
    process::{Command, Output},
};

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use tempfile::TempDir;
use test_support::{fs as test_fs, write_exec_with_content};

const CANDIDATE_REVISION: &str = "a1b2c3d4";
const CANDIDATE_VERSION: &str = "0.1.0-beta2";
const TEST_PATH: &str = "/usr/bin:/bin";

struct InstallerHarness {
    root: TempDir,
    bash_env_path: PathBuf,
    fake_bin_dir: PathBuf,
    cargo_args_path: PathBuf,
    github_output_path: PathBuf,
}

impl InstallerHarness {
    fn new() -> Result<Self> {
        let root = TempDir::new().context("create installer test directory")?;
        let fake_bin_dir = root.path().join("fake-bin");
        test_fs::create_dir(&fake_bin_dir).context("create fake command directory")?;
        let cargo_args_path = root.path().join("cargo-args");
        let github_output_path = root.path().join("github-output");
        let bash_env_path = root.path().join("bash-env");
        test_fs::write(&bash_env_path, "").context("write empty Bash environment")?;

        write_exec_with_content(&fake_bin_dir, "git", fake_git_script())?;
        write_exec_with_content(&fake_bin_dir, "cargo", fake_cargo_script())?;

        Ok(Self {
            root,
            bash_env_path,
            fake_bin_dir,
            cargo_args_path,
            github_output_path,
        })
    }

    fn run(&self, runner_os: &str, resolved_revision: &str, version: &str) -> Result<Output> {
        Command::new("bash")
            .arg(installer_script())
            .env("BASH_ENV", &self.bash_env_path)
            .env("GITHUB_OUTPUT", &self.github_output_path)
            .env("NETSUKE_CANDIDATE_REVISION", CANDIDATE_REVISION)
            .env("NETSUKE_CANDIDATE_VERSION", CANDIDATE_VERSION)
            .env("NETSUKE_CARGO_ARGS", &self.cargo_args_path)
            .env("NETSUKE_FAKE_BINARY_NAME", binary_name(runner_os))
            .env("NETSUKE_FAKE_RESOLVED_REVISION", resolved_revision)
            .env("NETSUKE_FAKE_VERSION", version)
            .env(
                "PATH",
                format!("{}:{TEST_PATH}", self.fake_bin_dir.display()),
            )
            .env("RUNNER_OS", runner_os)
            .env("RUNNER_TEMP", self.root.path())
            .output()
            .context("run release-candidate installer")
    }

    fn outputs(&self) -> Result<BTreeMap<String, String>> {
        let contents = test_fs::read_to_string(&self.github_output_path)
            .context("read installer GitHub output")?;
        Ok(contents
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect())
    }

    fn cargo_args(&self) -> Result<String> {
        test_fs::read_to_string(&self.cargo_args_path).context("read recorded cargo arguments")
    }
}

fn installer_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".github/actions/install-release-candidate/install.sh")
}

fn binary_name(runner_os: &str) -> &'static str {
    if runner_os == "Windows" {
        "netsuke.exe"
    } else {
        "netsuke"
    }
}

const fn fake_git_script() -> &'static str {
    r#"#!/usr/bin/env bash
set -euo pipefail

if [[ "$1" == "-C" ]]; then
  shift 2
fi

case "$1" in
  init|remote|fetch|checkout) exit 0 ;;
  rev-parse) printf '%s\n' "${NETSUKE_FAKE_RESOLVED_REVISION}" ;;
  *) echo "unexpected git invocation: $*" >&2; exit 1 ;;
esac
"#
}

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

#[test]
fn composite_action_invokes_the_tested_installer_script() -> Result<()> {
    let action = test_fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".github/actions/install-release-candidate/action.yml"),
    )
    .context("read release-candidate composite action")?;

    ensure!(
        action.contains("bash \"${GITHUB_ACTION_PATH}/install.sh\""),
        "the composite action should invoke the installer covered by this harness"
    );

    Ok(())
}

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

#[test]
fn installer_rejects_a_resolved_revision_that_differs_from_the_candidate() -> Result<()> {
    let harness = InstallerHarness::new()?;

    let output = harness.run("Linux", "different-revision", CANDIDATE_VERSION)?;

    ensure!(
        !output.status.success(),
        "installer should reject the wrong revision"
    );
    ensure!(String::from_utf8_lossy(&output.stderr).contains("candidate revision mismatch"));
    ensure!(
        !harness.cargo_args_path.exists(),
        "installer should not build an unverified candidate"
    );

    Ok(())
}

#[test]
fn installer_rejects_a_candidate_binary_with_the_wrong_version() -> Result<()> {
    let harness = InstallerHarness::new()?;

    let output = harness.run("Linux", CANDIDATE_REVISION, "0.1.0-wrong")?;

    ensure!(
        !output.status.success(),
        "installer should reject the wrong version"
    );
    ensure!(String::from_utf8_lossy(&output.stderr).contains("candidate version mismatch"));
    ensure!(
        harness.cargo_args_path.exists(),
        "installer should build only after it verifies the revision"
    );

    Ok(())
}
