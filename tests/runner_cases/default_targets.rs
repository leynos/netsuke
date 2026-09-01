//! Unix-only runner tests covering CLI default-target execution.
//!
//! The whole crate is Unix-only: it drives a fake `ninja` shell script and
//! the Unix-only `FakeNinjaFixture`, neither of which exists on Windows.

#![cfg(unix)]

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use netsuke::cli::{BuildArgs, Cli, Commands};
use netsuke::output_prefs;
use netsuke::runner::run_with_ninja_program;
use rstest::{fixture, rstest};
use std::path::PathBuf;

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
