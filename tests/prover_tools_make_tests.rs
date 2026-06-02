//! Behavioural tests for delegated prover-tool Make targets.
//!
//! These tests run the repository Makefile with a fake `prover-tools`
//! executable so they can verify delegation, diagnostic redaction, and failure
//! propagation without installing Kani or Verus.

#![cfg(unix)]

use anyhow::{Context, Result, ensure};
use rstest::{fixture, rstest};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::{TempDir, tempdir};

struct FakeProverTools {
    _temp: TempDir,
    executable: PathBuf,
    invocation_log: PathBuf,
    version_file: PathBuf,
}

impl FakeProverTools {
    fn new() -> Result<Self> {
        let temp = tempdir().context("create fake prover-tools directory")?;
        let executable = temp.path().join("prover-tools");
        let invocation_log = temp.path().join("invocations.log");
        let version_file = temp.path().join("KANI_VERSION");
        let pinned_version = fs::read_to_string("tools/kani/VERSION")
            .context("read pinned Kani version from tools/kani/VERSION")?;
        fs::write(&version_file, pinned_version.trim()).context("write fake Kani version file")?;
        write_fake_prover_tools(&executable, &invocation_log)?;

        Ok(Self {
            _temp: temp,
            executable,
            invocation_log,
            version_file,
        })
    }

    fn invocations(&self) -> Result<String> {
        fs::read_to_string(&self.invocation_log)
            .with_context(|| format!("read {}", self.invocation_log.display()))
    }
}

#[derive(Copy, Clone, Debug)]
struct ProverMakeCase {
    target: &'static str,
    expected_args: &'static str,
    secret_flags_variable: &'static str,
    secret_flags_value: &'static str,
}

impl ProverMakeCase {
    const fn install_kani() -> Self {
        Self {
            target: "install-kani",
            expected_args: "kani install --api-token=kani-install-secret",
            secret_flags_variable: "KANI_INSTALL_FLAGS",
            secret_flags_value: "--api-token=kani-install-secret",
        }
    }

    const fn kani_check() -> Self {
        Self {
            target: "kani-check",
            expected_args: "kani check-version --kani-command cargo kani --license=kani-check-secret",
            secret_flags_variable: "KANI_CHECK_FLAGS",
            secret_flags_value: "--license=kani-check-secret",
        }
    }

    const fn install_verus() -> Self {
        Self {
            target: "install-verus",
            expected_args: "verus install --token=verus-install-secret",
            secret_flags_variable: "VERUS_INSTALL_FLAGS",
            secret_flags_value: "--token=verus-install-secret",
        }
    }

    const fn verus() -> Self {
        Self {
            target: "verus",
            expected_args: "verus run --proof-token=verus-run-secret",
            secret_flags_variable: "VERUS_FLAGS",
            secret_flags_value: "--proof-token=verus-run-secret",
        }
    }
}

fn write_fake_prover_tools(executable: &Path, invocation_log: &Path) -> Result<()> {
    let script = format!(
        r#"#!/bin/sh
printf '%s\n' "$*" >> '{}'
exit "${{PROVER_TOOLS_EXIT:-0}}"
"#,
        invocation_log.display()
    );
    fs::write(executable, script).with_context(|| {
        format!(
            "write fake prover-tools executable to {}",
            executable.display()
        )
    })?;
    let mut permissions = fs::metadata(executable)
        .with_context(|| format!("stat fake prover-tools executable {}", executable.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(executable, permissions)
        .with_context(|| format!("mark {} executable", executable.display()))
}

#[fixture]
fn fake_prover_tools() -> Result<FakeProverTools> {
    FakeProverTools::new()
}

fn make_command(fake: &FakeProverTools) -> Command {
    let mut command = Command::new("make");
    command
        .arg("--no-print-directory")
        .arg("-f")
        .arg("Makefile")
        .arg(format!("PROVER_TOOLS={}", fake.executable.display()))
        .arg("PROVER_TOOLS_SOURCE=fake-prover-tools-source")
        .arg(format!("KANI_VERSION_FILE={}", fake.version_file.display()))
        .arg("KANI=cargo kani");
    command
}

fn run_make_target(fake: &FakeProverTools, case: ProverMakeCase) -> Result<Output> {
    let output = make_command(fake)
        .arg(case.target)
        .arg(format!(
            "{}={}",
            case.secret_flags_variable, case.secret_flags_value
        ))
        .output()
        .with_context(|| format!("run make {}", case.target))?;
    Ok(output)
}

#[rstest]
#[case(ProverMakeCase::install_kani())]
#[case(ProverMakeCase::kani_check())]
#[case(ProverMakeCase::install_verus())]
#[case(ProverMakeCase::verus())]
fn delegated_prover_targets_call_tool_and_redact_secret_flags(
    fake_prover_tools: Result<FakeProverTools>,
    #[case] case: ProverMakeCase,
) -> Result<()> {
    let fake = fake_prover_tools?;

    let output = run_make_target(&fake, case)?;
    ensure!(
        output.status.success(),
        "make {} should succeed: {}",
        case.target,
        String::from_utf8_lossy(&output.stderr)
    );

    let invocations = fake.invocations()?;
    ensure!(
        invocations.contains(case.expected_args),
        "fake prover-tools should receive real arguments `{}`, got `{}`",
        case.expected_args,
        invocations
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        stderr.contains("<redacted-flags>"),
        "diagnostics should show a redacted flag placeholder, got `{stderr}`"
    );
    ensure!(
        !stderr.contains(case.secret_flags_value),
        "diagnostics should not leak secret flags, got `{stderr}`"
    );
    Ok(())
}

#[test]
fn delegated_prover_target_reports_failure_exit_status() -> Result<()> {
    let fake = FakeProverTools::new()?;
    let output = make_command(&fake)
        .arg("install-verus")
        .arg("VERUS_INSTALL_FLAGS=--token=verus-install-secret")
        .env("PROVER_TOOLS_EXIT", "42")
        .output()
        .context("run failing make install-verus")?;

    ensure!(!output.status.success(), "make install-verus should fail");

    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        stderr.contains("prover-tools: target=install-verus failed exit=42"),
        "diagnostics should report the failing target and exit status, got `{stderr}`"
    );
    ensure!(
        !stderr.contains("verus-install-secret"),
        "failure diagnostics should not leak secret flags, got `{stderr}`"
    );
    Ok(())
}
