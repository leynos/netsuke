//! Behavioural contract for the documented Windows `PATH` setup.
//!
//! The users' guide fences are also checked structurally, but only executing
//! them shows that the write lands: PowerShell 7 opens the registry provider
//! key read-only, so an append that reads correctly still throws. This test
//! runs each documented script against a temporary install directory and
//! inspects the user environment value the run leaves behind.
//!
//! The wrapper owns the registry key, not the script: it records the existing
//! value, installs a sentinel, dot-sources the documented example, reports the
//! stored value and its type, and restores the original value in a `finally`
//! block whatever the example does.
//!
//! The contract is only observable on Windows, so the case is ignored
//! elsewhere rather than passing without doing anything. Keeping the file
//! compiled on every host means a mistake in the harness is caught by the
//! Linux gates instead of the Windows job.

pub mod documentation_examples;

use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

use anyhow::{Context, Result, bail, ensure};
use documentation_examples::documented_example;
use test_support::fs as test_fs;

/// Sentinel user `PATH` value every run starts from.
///
/// It is an expandable string, so the example's promise to preserve the value's
/// type is observable: a script that rewrote `PATH` as a plain string would
/// fail the assertion below even though the directory was added.
const SENTINEL_PATH: &str = "%SystemRoot%\\netsuke-path-sentinel";

/// The wrapper that surrounds the documented script.
///
/// It passes `-ExamplePath` to the script, owns the registry key for the
/// duration of the run, and reports the three observations the assertions use.
const WRAPPER: &str = r#"
param([Parameter(Mandatory)][string]$ExamplePath)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$environmentKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
if ($null -eq $environmentKey) {
    throw 'User environment registry key not found.'
}
$hadPath = $environmentKey.GetValueNames() -contains 'Path'
$originalValue = if ($hadPath) {
    [string]$environmentKey.GetValue('Path', '', 'DoNotExpandEnvironmentNames')
} else {
    ''
}
$originalKind = if ($hadPath) {
    $environmentKey.GetValueKind('Path')
} else {
    [Microsoft.Win32.RegistryValueKind]::ExpandString
}
Write-Output "ORIGINAL=$originalValue"
try {
    $environmentKey.SetValue(
        'Path',
        '%SystemRoot%\netsuke-path-sentinel',
        [Microsoft.Win32.RegistryValueKind]::ExpandString
    )
    . $ExamplePath
    $stored = [string]$environmentKey.GetValue('Path', '', 'DoNotExpandEnvironmentNames')
    Write-Output "RESULT=$stored"
    Write-Output "KIND=$($environmentKey.GetValueKind('Path'))"
}
finally {
    # Record what the script left behind before restoring it. A run that threw
    # never reaches the RESULT line, so LEFTOVER is the only evidence of
    # whether it changed the value on the way out.
    $leftover = [string]$environmentKey.GetValue('Path', '', 'DoNotExpandEnvironmentNames')
    Write-Output "LEFTOVER=$leftover"
    if ($hadPath) {
        $environmentKey.SetValue('Path', $originalValue, $originalKind)
    } else {
        $environmentKey.DeleteValue('Path', $false)
    }
    $restored = if ($environmentKey.GetValueNames() -contains 'Path') {
        [string]$environmentKey.GetValue('Path', '', 'DoNotExpandEnvironmentNames')
    } else {
        ''
    }
    Write-Output "RESTORED=$restored"
    $environmentKey.Dispose()
}
"#;

/// Verify that each documented script appends its directory to the user
/// `PATH` and keeps the value expandable, and that it changes nothing when the
/// executable is absent.
///
/// Both examples run inside one test on purpose: every run rewrites the same
/// user `PATH` value, so separate cases could execute concurrently and read
/// each other's value.
#[test]
#[cfg_attr(
    not(windows),
    ignore = "the documented scripts write the Windows user environment key"
)]
fn documented_path_setup_persists_the_directory() -> Result<()> {
    for example_id in ["guide-windows-path", "guide-windows-path-beta4"] {
        assert_example_persists_the_directory(example_id)?;
        assert_missing_executable_changes_nothing(example_id)?;
    }
    Ok(())
}

/// Run one documented example against a staged install and check the value it
/// stored.
fn assert_example_persists_the_directory(example_id: &str) -> Result<()> {
    let root = workspace(example_id)?;
    let install = install_directory(root.path())?;
    stage_binary(&install)?;
    let output = run_documented_example(&root, example_id)?;
    assert_persisted(&output, &install, example_id)
}

/// Run one documented example without an installed executable and check that
/// it stopped before writing.
///
/// The existence guard is the only thing standing between a user who has not
/// installed Netsuke and a `PATH` entry pointing at nothing, so the failure
/// path is observed rather than inferred from the script's text.
fn assert_missing_executable_changes_nothing(example_id: &str) -> Result<()> {
    let root = workspace(example_id)?;
    let install = install_directory(root.path())?;
    let output = run_documented_example(&root, example_id)?;
    assert_untouched(&output, example_id)?;
    ensure!(
        !output.status.success(),
        "{example_id} should fail when {} holds no netsuke.exe, exited with {}",
        install.display(),
        output.status
    );
    Ok(())
}

/// Create a temporary workspace for one run of `example_id`.
fn workspace(example_id: &str) -> Result<tempfile::TempDir> {
    tempfile::tempdir().with_context(|| format!("create workspace for {example_id}"))
}

/// Write the documented example and the registry wrapper into `root` and run
/// them, returning the wrapper's output.
fn run_documented_example(root: &tempfile::TempDir, example_id: &str) -> Result<Output> {
    let example = documented_example(example_id)?;
    ensure!(
        example.language == "powershell",
        "{example_id} should be a PowerShell example, got {:?}",
        example.language
    );
    let example_script = root.path().join("example.ps1");
    test_fs::write(&example_script, example.body)
        .with_context(|| format!("write {example_id} to {}", example_script.display()))?;
    let wrapper = root.path().join("wrapper.ps1");
    test_fs::write(&wrapper, WRAPPER).context("write the registry wrapper")?;
    run_wrapper(&wrapper, &example_script, root.path())
}

/// Create the directory the examples treat as the install root.
///
/// The directory is returned because the assertions compare the stored value
/// against it.
fn install_directory(root: &Path) -> Result<PathBuf> {
    let install = root.join("ProgramFiles").join("netsuke");
    test_fs::create_dir_all(&install)
        .with_context(|| format!("create install directory {}", install.display()))?;
    Ok(install)
}

/// Copy the built executable into a staged install directory.
///
/// The examples refuse to run when the executable is absent, so this copy is
/// what makes a fence reach its `PATH` work.
fn stage_binary(install: &Path) -> Result<()> {
    let binary = Path::new(env!("CARGO_BIN_EXE_netsuke"));
    let staged = install.join("netsuke.exe");
    test_fs::copy(binary, &staged)
        .with_context(|| format!("copy {} to {}", binary.display(), staged.display()))?;
    Ok(())
}

/// Run the wrapper under the first PowerShell the host provides.
///
/// PowerShell 7 (`pwsh`) is the documented shell, but Windows PowerShell
/// (`powershell.exe`) ships with every Windows image and drives the same
/// registry APIs, so a host without `pwsh` still exercises the script instead
/// of failing on a missing executable.
fn run_wrapper(wrapper: &Path, example: &Path, program_files_root: &Path) -> Result<Output> {
    let mut last_error = None;
    for shell in ["pwsh", "powershell"] {
        let output = Command::new(shell)
            .args(["-NoProfile", "-NonInteractive", "-File"])
            .arg(wrapper)
            .arg("-ExamplePath")
            .arg(example)
            // Every variable the examples consult points at the temporary
            // tree, so neither script can resolve the host's real directories.
            .env("ProgramW6432", program_files_root)
            .env("ProgramFiles", program_files_root)
            .env("ProgramFiles(x86)", program_files_root)
            .output();
        match output {
            Ok(output) => return Ok(output),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error
        .map(anyhow::Error::from)
        .unwrap_or_else(|| anyhow::anyhow!("no PowerShell executable was available"))
        .context("run the documented PATH setup"))
}

/// Assert that the run stored the staged directory and kept the value type.
fn assert_persisted(output: &Output, install: &Path, example_id: &str) -> Result<()> {
    let stdout = require_success(output, example_id)?;
    let stored = marker(&stdout, "RESULT")
        .with_context(|| describe(output, example_id, "did not report RESULT"))?;
    let kind = marker(&stdout, "KIND")
        .with_context(|| describe(output, example_id, "did not report KIND"))?;
    let expected = format!("{SENTINEL_PATH};{}", install.display());
    ensure!(
        stored == expected,
        "{example_id} should append {} to the raw user PATH, stored {stored:?}",
        install.display()
    );
    ensure!(
        kind == "ExpandString",
        "{example_id} should preserve the PATH value type, stored {kind:?}"
    );
    assert_restored(output, example_id)
}

/// Assert that a run which had to fail left the user `PATH` alone.
fn assert_untouched(output: &Output, example_id: &str) -> Result<()> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        marker(&stdout, "RESULT").is_none(),
        "{}",
        describe(
            output,
            example_id,
            "changed the user PATH despite the missing executable"
        )
    );
    let leftover = marker(&stdout, "LEFTOVER")
        .with_context(|| describe(output, example_id, "did not report LEFTOVER"))?;
    ensure!(
        leftover == SENTINEL_PATH,
        "{}",
        describe(
            output,
            example_id,
            &format!("should leave the user PATH at the sentinel, left {leftover:?}")
        )
    );
    assert_restored(output, example_id)
}

/// Return the wrapper's standard output, or the whole run as a failure.
fn require_success(output: &Output, example_id: &str) -> Result<String> {
    if !output.status.success() {
        bail!("{}", describe(output, example_id, "failed"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Assert that the wrapper put the original value back.
fn assert_restored(output: &Output, example_id: &str) -> Result<()> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let original = marker(&stdout, "ORIGINAL")
        .with_context(|| describe(output, example_id, "did not report ORIGINAL"))?;
    let restored = marker(&stdout, "RESTORED")
        .with_context(|| describe(output, example_id, "did not report RESTORED"))?;
    ensure!(
        restored == original,
        "{example_id} wrapper should restore the original PATH, {restored:?} vs {original:?}"
    );
    Ok(())
}

/// Describe a run in enough detail to diagnose it.
///
/// The wrapper's output is the only evidence of what the script did, so every
/// failure carries it rather than just the missing marker.
fn describe(output: &Output, example_id: &str, detail: &str) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!(
        "{example_id} {detail}; wrapper exited with {}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        output.status
    )
}

/// Read the value of a `NAME=value` line the wrapper printed.
fn marker<'a>(stdout: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{name}=");
    stdout
        .lines()
        .find_map(|line| line.strip_prefix(prefix.as_str()))
}
