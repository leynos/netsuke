//! Behaviourally covers the mutation-gate Makefile prerequisites.
//!
//! The tests run the repository Makefile from a disposable directory with
//! logging stubs for every external tool. That proves the prerequisite graph
//! executes the gate before the expensive test and lint tools without running
//! Cargo or modifying the checkout.

use anyhow::{Context, Result, ensure};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::{TempDir, tempdir};
use test_support::write_exec_with_content;

/// Hold one disposable Makefile workspace and its external-tool stubs.
struct MakeFixture {
    /// Temporary directory containing the isolated Makefile working directory.
    workspace: TempDir,
    /// Stubbed Python runner used by the mutation gate.
    uv: PathBuf,
    /// Stubbed Cargo command used by test, lint, and typecheck recipes.
    cargo: PathBuf,
    /// Stubbed Whitaker command used by lint recipes.
    whitaker: PathBuf,
    /// Log written by every stubbed external command.
    log: PathBuf,
    /// Repository Makefile under test.
    makefile: PathBuf,
}

impl MakeFixture {
    /// Create an isolated Makefile workspace with logging tool stubs.
    fn new() -> Result<Self> {
        let workspace = tempdir().context("create disposable Makefile fixture")?;
        let bin = workspace.path().join("bin");
        test_support::fs::create_dir(&bin).context("create disposable stub directory")?;
        test_support::fs::create_dir(workspace.path().join("test_support"))
            .context("create lint target's test-support directory")?;
        test_support::fs::write(workspace.path().join("dylint.toml"), "")
            .context("write lint target's empty Dylint configuration")?;
        test_support::fs::write(
            workspace.path().join("test_support").join("dylint.toml"),
            "",
        )
        .context("write test-support lint target's empty Dylint configuration")?;
        for target in ["test-env-mutation-gate", "lint-env-mutation"] {
            test_support::fs::write(workspace.path().join(target), "")
                .with_context(|| format!("create conflicting regular file for {target}"))?;
        }
        let log = workspace.path().join("invocations.log");
        let uv = write_logging_stub(&bin, "uv")?;
        let cargo = write_logging_stub(&bin, "cargo")?;
        let whitaker = write_logging_stub(&bin, "whitaker")?;
        let makefile = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Makefile");
        Ok(Self {
            workspace,
            uv,
            cargo,
            whitaker,
            log,
            makefile,
        })
    }

    /// Run one repository Makefile target with every external tool stubbed.
    fn run(&self, target: &str) -> Result<Output> {
        Command::new("make")
            .arg("--no-print-directory")
            .arg("-f")
            .arg(&self.makefile)
            .arg(format!("UV={}", self.uv.display()))
            .arg(format!("CARGO={}", self.cargo.display()))
            .arg(format!("WHITAKER={}", self.whitaker.display()))
            .arg(target)
            .current_dir(self.workspace.path())
            .env("MAKE_INVOCATIONS", &self.log)
            .output()
            .with_context(|| format!("run make {target} with disposable tool stubs"))
    }

    /// Read every external-tool invocation recorded by the fixture.
    fn invocations(&self) -> Result<String> {
        test_support::fs::read_to_string(&self.log).context("read stubbed Makefile invocations")
    }

    /// Clear the external-tool invocation log between Make targets.
    fn clear_invocations(&self) -> Result<()> {
        test_support::fs::write(&self.log, "").context("clear Makefile invocation log")
    }
}

/// Write a shell stub that appends its name and arguments to the supplied log.
fn write_logging_stub(directory: &Path, name: &str) -> Result<PathBuf> {
    let content =
        format!("#!/bin/sh\nprintf '%s:%s\\n' '{name}' \"$*\" >> \"$MAKE_INVOCATIONS\"\n");
    write_exec_with_content(directory, name, &content)
        .with_context(|| format!("write {name} Makefile-tool stub"))
}

/// Verify that `output` reports a successful stubbed Make target.
fn assert_target_succeeds(target: &str, output: &Output) -> Result<()> {
    ensure!(
        output.status.success(),
        "stubbed make {target} should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

/// Verify that phony mutation gates execute before the remaining gate tools.
#[test]
fn mutation_gate_prerequisites_execute_in_an_isolated_make_fixture() -> Result<()> {
    let fixture = MakeFixture::new()?;
    let test = fixture.run("test")?;
    assert_target_succeeds("test", &test)?;
    let test_invocations = fixture.invocations()?;
    ensure!(
        test_invocations
            .lines()
            .next()
            .is_some_and(|line| line.starts_with("uv:run")),
        "test should run the mutation gate first, got {test_invocations:?}"
    );
    ensure!(
        test_invocations.contains("cargo:nextest run"),
        "test should run nextest after the mutation gate, got {test_invocations:?}"
    );
    ensure!(
        test_invocations.contains("cargo:test --workspace --doc"),
        "test should run doctests after the mutation gate, got {test_invocations:?}"
    );

    fixture.clear_invocations()?;
    let lint = fixture.run("lint")?;
    assert_target_succeeds("lint", &lint)?;
    let lint_invocations = fixture.invocations()?;
    ensure!(
        lint_invocations
            .lines()
            .next()
            .is_some_and(|line| line.starts_with("uv:run")),
        "lint should run the mutation gate first, got {lint_invocations:?}"
    );
    ensure!(
        lint_invocations.contains("cargo:doc --workspace --no-deps"),
        "lint should run rustdoc after the mutation gate, got {lint_invocations:?}"
    );
    ensure!(
        lint_invocations.contains("cargo:clippy"),
        "lint should run Clippy after the mutation gate, got {lint_invocations:?}"
    );
    ensure!(
        lint_invocations
            .lines()
            .filter(|line| line.starts_with("whitaker:"))
            .count()
            == 2,
        "lint should run both Whitaker scopes after the mutation gate, got {lint_invocations:?}"
    );
    Ok(())
}
