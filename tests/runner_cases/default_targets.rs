//! Unix-only runner tests covering CLI default-target execution.
//!
//! The whole crate is Unix-only: it drives a fake `ninja` shell script and
//! the Unix-only `FakeNinjaFixture`, neither of which exists on Windows.

#![cfg(unix)]

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use netsuke::cli::{BuildArgs, Cli, Commands};
use netsuke::output_prefs;
use netsuke::runner::{
    BuildTargets, CommandEnv, NinjaBuildRequest, NinjaProcessOptions, StderrMode, run_ninja_with,
    run_with_ninja_program,
};
use rstest::{fixture, rstest};
use std::path::PathBuf;
use test_support::ninja_gen;

use crate::fixtures::create_test_manifest;

/// Test fixture that installs a temporary fake `ninja` environment.
///
/// `_ninja_dir` owns the temporary directory containing the generated fake
/// `ninja` binary and log file, `_guard` restores the overridden ninja
/// environment on drop, and `args_log` points to the recorded invocation
/// arguments emitted by the fake binary.
#[cfg(unix)]
struct FakeNinjaFixture {
    _ninja_dir: tempfile::TempDir,
    ninja_path: PathBuf,
    args_log: PathBuf,
}

/// Creates a [`FakeNinjaFixture`] used to simulate ninja behaviour for tests,
/// returning a temporary environment and the recorded invocation log path.
#[cfg(unix)]
#[fixture]
fn fake_ninja_fixture() -> Result<FakeNinjaFixture> {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let ninja_dir = tempfile::tempdir().context("create fake ninja directory")?;
    let args_log = ninja_dir.path().join("ninja-args.log");
    let ninja_path = ninja_dir.path().join("ninja");
    fs::write(
        &ninja_path,
        format!(
            "#!/bin/sh\nprintf '%s\n' \"$@\" > \"{}\"\nexit 0\n",
            args_log.display()
        ),
    )
    .with_context(|| format!("write fake ninja script {}", ninja_path.display()))?;
    let mut permissions = fs::metadata(&ninja_path)
        .with_context(|| format!("read fake ninja metadata {}", ninja_path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&ninja_path, permissions)
        .with_context(|| format!("chmod fake ninja {}", ninja_path.display()))?;

    Ok(FakeNinjaFixture {
        _ninja_dir: ninja_dir,
        ninja_path,
        args_log,
    })
}

#[cfg(unix)]
#[rstest]
fn run_build_uses_cli_default_targets_when_no_targets_are_requested(
    fake_ninja_fixture: Result<FakeNinjaFixture>,
) -> Result<()> {
    use std::fs;

    let fixture = fake_ninja_fixture?;
    let (temp, manifest_path) = create_test_manifest()?;
    let cli = Cli {
        file: Utf8PathBuf::from_path_buf(manifest_path).map_err(|non_utf8| {
            anyhow::anyhow!("manifest path is not valid UTF-8: {}", non_utf8.display())
        })?,
        directory: Some(
            Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).map_err(|non_utf8| {
                anyhow::anyhow!(
                    "temporary directory is not valid UTF-8: {}",
                    non_utf8.display()
                )
            })?,
        ),
        default_targets: vec![String::from("hello")],
        command: Some(Commands::Build(BuildArgs {
            targets: Vec::new(),
        })),
        ..Cli::default()
    };

    run_with_ninja_program(
        &cli,
        output_prefs::resolve(None),
        Utf8Path::from_path(&fixture.ninja_path).context("fake ninja path is not valid UTF-8")?,
    )
    .context("run build with cli default targets")?;

    let logged_args = fs::read_to_string(&fixture.args_log)
        .with_context(|| format!("read fake ninja args log {}", fixture.args_log.display()))?;
    ensure!(
        logged_args.lines().any(|line| line == "hello"),
        "expected fake ninja invocation to include default target 'hello', got: {logged_args}"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn configured_targets_cannot_replace_the_generated_build_file(
    fake_ninja_fixture: Result<FakeNinjaFixture>,
) -> Result<()> {
    use std::fs;

    let fixture = fake_ninja_fixture?;
    let (temp, manifest_path) = create_test_manifest()?;
    let cli = Cli {
        file: Utf8PathBuf::from_path_buf(manifest_path).map_err(|non_utf8| {
            anyhow::anyhow!("manifest path is not valid UTF-8: {}", non_utf8.display())
        })?,
        directory: Some(
            Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).map_err(|non_utf8| {
                anyhow::anyhow!(
                    "temporary directory is not valid UTF-8: {}",
                    non_utf8.display()
                )
            })?,
        ),
        default_targets: vec![String::from("-f"), String::from("evil.ninja")],
        command: Some(Commands::Build(BuildArgs {
            targets: Vec::new(),
        })),
        ..Cli::default()
    };

    run_with_ninja_program(
        &cli,
        output_prefs::resolve(None),
        Utf8Path::from_path(&fixture.ninja_path).context("fake ninja path is not valid UTF-8")?,
    )
    .context("run build with configured option-like targets")?;

    let logged_args = fs::read_to_string(&fixture.args_log)
        .with_context(|| format!("read fake ninja args log {}", fixture.args_log.display()))?;
    let arguments: Vec<&str> = logged_args.lines().collect();
    ensure!(
        arguments.ends_with(&["--", "-f", "evil.ninja"]),
        "configured targets must follow '--', got: {arguments:?}"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn explicit_targets_cannot_change_ninjas_working_directory(
    fake_ninja_fixture: Result<FakeNinjaFixture>,
) -> Result<()> {
    use std::fs;

    let fixture = fake_ninja_fixture?;
    let (temp, manifest_path) = create_test_manifest()?;
    let cli = Cli {
        file: Utf8PathBuf::from_path_buf(manifest_path).map_err(|non_utf8| {
            anyhow::anyhow!("manifest path is not valid UTF-8: {}", non_utf8.display())
        })?,
        directory: Some(
            Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).map_err(|non_utf8| {
                anyhow::anyhow!(
                    "temporary directory is not valid UTF-8: {}",
                    non_utf8.display()
                )
            })?,
        ),
        command: Some(Commands::Build(BuildArgs {
            targets: vec![
                String::from("-C"),
                String::from("evil"),
                String::from("default"),
            ],
        })),
        ..Cli::default()
    };

    run_with_ninja_program(
        &cli,
        output_prefs::resolve(None),
        Utf8Path::from_path(&fixture.ninja_path).context("fake ninja path is not valid UTF-8")?,
    )
    .context("run build with explicit option-like targets")?;

    let logged_args = fs::read_to_string(&fixture.args_log)
        .with_context(|| format!("read fake ninja args log {}", fixture.args_log.display()))?;
    let arguments: Vec<&str> = logged_args.lines().collect();
    ensure!(
        arguments.ends_with(&["--", "-C", "evil", "default"]),
        "explicit targets must follow '--', got: {arguments:?}"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn real_ninja_treats_option_like_targets_as_operands() -> Result<()> {
    use std::fs;

    let Some(workspace) = ninja_gen::ninja_integration_setup() else {
        return Ok(());
    };
    let trusted_dir = workspace.path().join("trusted");
    let evil_dir = workspace.path().join("evil");
    fs::create_dir(&trusted_dir)
        .with_context(|| format!("create trusted directory {}", trusted_dir.display()))?;
    fs::create_dir(&evil_dir)
        .with_context(|| format!("create attacker directory {}", evil_dir.display()))?;
    let trusted_build_file = trusted_dir.join("build.ninja");
    fs::write(
        &trusted_build_file,
        concat!(
            "rule touch\n",
            "  command = touch $out\n",
            "build safe-output: touch\n",
            "build safe: phony safe-output\n",
            "build -C: phony\n",
            "build ../evil: phony\n",
            "build default: phony\n"
        ),
    )
    .with_context(|| format!("write trusted build file {}", trusted_build_file.display()))?;
    fs::write(
        evil_dir.join("build.ninja"),
        concat!(
            "rule attack\n",
            "  command = touch attacker-output\n",
            "build safe: attack\n",
            "default safe\n"
        ),
    )
    .context("write attacker-controlled build file")?;
    let targets = vec![
        String::from("safe"),
        String::from("-C"),
        String::from("../evil"),
        String::from("default"),
    ];
    let build_targets = BuildTargets::new(&targets);
    let options = NinjaProcessOptions {
        working_dir: Some(
            Utf8PathBuf::from_path_buf(trusted_dir.clone())
                .map_err(|path| anyhow::anyhow!("trusted path is not UTF-8: {}", path.display()))?,
        ),
        ..NinjaProcessOptions::default()
    };
    let build_file = Utf8PathBuf::from_path_buf(trusted_build_file).map_err(|path| {
        anyhow::anyhow!("trusted build file path is not UTF-8: {}", path.display())
    })?;
    let env = CommandEnv::inherit();
    let request = NinjaBuildRequest {
        program: Utf8Path::new("ninja"),
        options: &options,
        build_file: &build_file,
        targets: &build_targets,
        env: &env,
        stderr_mode: StderrMode::Forward,
    };

    run_ninja_with(&request).context("run real ninja with option-like target operands")?;

    ensure!(
        trusted_dir.join("safe-output").is_file(),
        "trusted build recipe should run in the trusted directory"
    );
    ensure!(
        !evil_dir.join("safe-output").exists(),
        "the -C operand must not redirect Ninja into the attacker directory"
    );
    ensure!(
        !evil_dir.join("attacker-output").exists(),
        "the attacker-controlled build file must not run"
    );
    Ok(())
}
