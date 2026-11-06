//! Verify Windows PowerShell binding diagnostics at the runner boundary.

use std::path::{Path, PathBuf};

use super::resolve_power_shell_runtime_with;
use crate::power_shell_location::SystemDirectory;

/// Supply one fixed system directory to the locator.
struct FixedSystemDirectory(Option<PathBuf>);

impl SystemDirectory for FixedSystemDirectory {
    fn system_directory(&self) -> Option<PathBuf> {
        self.0.clone()
    }
}

/// Return the engine directory beneath one fixture system directory.
fn engine_directory(system_directory: &Path) -> PathBuf {
    system_directory.join("WindowsPowerShell").join("v1.0")
}

/// Create a regular `powershell.exe` beneath `system_directory`.
fn write_power_shell(system_directory: &Path) -> anyhow::Result<()> {
    let directory = engine_directory(system_directory);
    test_support::fs::create_dir_all(&directory)?;
    test_support::fs::write(directory.join("powershell.exe"), b"fixture executable")?;
    Ok(())
}

#[test]
fn binds_the_fixture_power_shell() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    write_power_shell(workspace.path())?;

    let binding = resolve_power_shell_runtime_with(&FixedSystemDirectory(Some(
        workspace.path().to_path_buf(),
    )))?;

    anyhow::ensure!(
        binding
            .as_path()
            .ends_with("WindowsPowerShell/v1.0/powershell.exe"),
        "binding should name the trusted engine path, got {}",
        binding.as_path()
    );
    Ok(())
}

#[test]
fn missing_power_shell_diagnostic_names_the_trusted_path_and_advice() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    test_support::fs::create_dir_all(engine_directory(workspace.path()))?;

    let error = resolve_power_shell_runtime_with(&FixedSystemDirectory(Some(
        workspace.path().to_path_buf(),
    )))
    .expect_err("a missing engine file should fail");

    let message = error.to_string();
    anyhow::ensure!(
        message.contains("WindowsPowerShell"),
        "diagnostic should name the trusted engine path: {message}"
    );
    anyhow::ensure!(
        message.contains("never searches PATH"),
        "diagnostic should state that PATH is not consulted: {message}"
    );
    anyhow::ensure!(
        message.contains("NETSUKE_WINDOWS_SHELL=bash"),
        "diagnostic should name the Bash override: {message}"
    );
    Ok(())
}

#[test]
fn unavailable_system_directory_diagnostic_names_the_fixed_engine_path() {
    let error = resolve_power_shell_runtime_with(&FixedSystemDirectory(None))
        .expect_err("an unavailable system directory should fail");

    let message = error.to_string();
    assert!(
        message.contains("WindowsPowerShell\\v1.0\\powershell.exe"),
        "diagnostic should name the fixed engine path: {message}"
    );
    assert!(
        message.contains("NETSUKE_WINDOWS_SHELL=bash"),
        "diagnostic should name the Bash override: {message}"
    );
}
