//! Regression tests for prover-tool command and installation edge cases.
//!
//! These tests exercise shell boundaries in the repository Makefile with a
//! fake `prover-tools` command and temporary Cargo installations.

#![cfg(unix)]

use anyhow::{Context, Result, ensure};
use cap_std::ambient_authority;
use cap_std::fs::{Dir, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::{TempDir, tempdir};

/// Write an executable shell script at `path`.
///
/// # Errors
///
/// Returns an error if the script cannot be written or marked executable.
fn write_executable(directory: &Dir, path: &str, script: &str) -> Result<()> {
    directory
        .write(path, script)
        .with_context(|| format!("write executable {path}"))?;
    let mut permissions = directory
        .metadata(path)
        .with_context(|| format!("stat executable {path}"))?
        .permissions();
    permissions.set_mode(0o755);
    directory
        .set_permissions(path, permissions)
        .with_context(|| format!("mark {path} executable"))
}

/// Create a fake prover-tools command and its argument log.
///
/// The fake invokes `cargo kani setup` for the compile installer path so Cargo
/// resolves the external subcommand using the Make target's effective `PATH`.
///
/// # Errors
///
/// Returns an error if the fake command cannot be created.
fn write_fake_prover_tools(temp: &TempDir, directory: &Dir) -> Result<PathBuf> {
    let executable = temp.path().join("prover-tools");
    let invocation_log = temp.path().join("invocations.log");
    let script = format!(
        r#"#!/bin/sh
for argument do
    printf '%s\n' "$argument" >> '{}'
done
if [ "$1" = kani ] && [ "$2" = install ] && [ "${{3:-}}" = --compile ]; then
    cargo kani setup || exit "$?"
fi
exit "${{PROVER_TOOLS_EXIT:-0}}"
"#,
        invocation_log.display()
    );
    write_executable(directory, "prover-tools", &script)?;
    Ok(executable)
}

/// Construct a Make command using the fake prover-tools executable.
fn make_command(executable: &Path) -> Command {
    let mut command = Command::new("make");
    command
        .arg("--no-print-directory")
        .arg("-f")
        .arg("Makefile")
        .arg(format!("PROVER_TOOLS={}", executable.display()))
        .arg("PROVER_TOOLS_SOURCE=fake-prover-tools-source");
    command
}

/// Prefer the active Cargo home when the compiled Kani installer runs setup.
#[test]
fn kani_compile_install_prefers_active_cargo_home_for_subcommands() -> Result<()> {
    let temp = tempdir().context("create fake prover-tools directory")?;
    let directory = Dir::open_ambient_dir(temp.path(), ambient_authority())
        .context("open fake prover-tools directory")?;
    let prover_tools = write_fake_prover_tools(&temp, &directory)?;
    let cargo_home = temp.path().join("cargo's home with spaces");
    let active_marker = temp.path().join("active-kani-selected");
    let conflicting_marker = temp.path().join("conflicting-kani-selected");
    directory
        .create_dir_all("cargo's home with spaces/bin")
        .context("create active Cargo bin directory")?;
    directory
        .create_dir_all("build-tools/bin")
        .context("create build-tools bin directory")?;
    write_executable(
        &directory,
        "cargo's home with spaces/bin/cargo-kani",
        &format!("#!/bin/sh\nprintf active > '{}'\n", active_marker.display()),
    )?;
    write_executable(
        &directory,
        "build-tools/bin/cargo-kani",
        &format!(
            "#!/bin/sh\nprintf conflicting > '{}'\n",
            conflicting_marker.display()
        ),
    )?;

    let output = make_command(&prover_tools)
        .arg("install-kani")
        .arg("KANI_INSTALL_FLAGS=--compile")
        .env("CARGO_HOME", &cargo_home)
        .env("BUILD_TOOLS_PREFIX", temp.path().join("build-tools"))
        .output()
        .context("run make install-kani with --compile")?;

    ensure!(
        output.status.success(),
        "make install-kani should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        directory.exists("active-kani-selected"),
        "Cargo should resolve cargo-kani from the active Cargo home"
    );
    ensure!(
        !directory.exists("conflicting-kani-selected"),
        "Cargo must not resolve a conflicting cargo-kani from build-tools"
    );
    Ok(())
}
