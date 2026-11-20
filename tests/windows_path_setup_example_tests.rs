//! Behavioural contract for the documented Windows `PATH` setup.
//!
//! The users' guide fences are also checked structurally, but only executing
//! them shows that the write lands: PowerShell 7 opens the registry provider
//! key read-only, so an append that reads correctly still throws. This test
//! runs each documented script against a temporary install directory and
//! inspects the user environment value the run leaves behind.
//!
//! The wrapper in [`windows_path_setup`] owns the registry key, not the script:
//! it records the existing value, installs a sentinel, dot-sources the
//! documented example, reports the stored value and its type, and restores the
//! original value in a `finally` block whatever the example does.
//!
//! The contract is only observable on Windows, so the case is ignored
//! elsewhere rather than passing without doing anything. Keeping the file
//! compiled on every host means a mistake in the harness is caught by the
//! Linux gates instead of the Windows job.

pub mod documentation_examples;
pub mod windows_path_setup;

use std::{
    path::{Path, PathBuf},
    process::Output,
};

use anyhow::{Context, Result, bail, ensure};
use documentation_examples::{DocumentedExample, documented_example};
use test_support::fs as test_fs;
use windows_path_setup::{
    run_example, run_example_after_interrupted_append, wrapper::SENTINEL_PATH,
};

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
        let example = documented_example(example_id)?;
        assert_example_persists_the_directory(&example)?;
        assert_missing_executable_changes_nothing(&example)?;
    }
    Ok(())
}

/// Verify that a run killed after the example appends its directory is repaired
/// by the next run.
///
/// The recovery exists for exactly this crash. Without it the next run adopts
/// the sentinel-plus-directory value as the user's own original, and the
/// entries the backup recorded are lost for good. The crashed state is seeded
/// inside the wrapper, under its mutex, so no concurrent run can observe it,
/// and the seeded backup records the real value, so this probe restores the
/// user's `PATH` rather than a stand-in.
#[test]
#[cfg_attr(
    not(windows),
    ignore = "the documented scripts write the Windows user environment key"
)]
fn interrupted_append_is_recovered_from_the_backup() -> Result<()> {
    let root = tempfile::tempdir().context("create the recovery workspace")?;
    let appended = install_directory(root.path())?;
    let example = DocumentedExample {
        id: "recovery-probe".to_owned(),
        language: "powershell".to_owned(),
        body: "# The recovery under test runs before this script.\n".to_owned(),
    };
    let output = run_example_after_interrupted_append(root.path(), &example, &appended)?;
    let stdout = require_success(&output, &example)?;
    let seeded = marker(&stdout, "SEEDED")
        .with_context(|| describe(&output, &example, "did not report SEEDED"))?;
    let captured = marker(&stdout, "ORIGINAL")
        .with_context(|| describe(&output, &example, "did not report ORIGINAL"))?;
    ensure!(
        captured == seeded,
        "the interrupted value should be repaired before capture, captured \
         {captured:?} with {seeded:?} recorded"
    );
    ensure!(
        !captured.starts_with(SENTINEL_PATH),
        "the repaired value should not carry the sentinel, captured {captured:?}"
    );
    let restored = marker(&stdout, "RESTORED")
        .with_context(|| describe(&output, &example, "did not report RESTORED"))?;
    ensure!(
        restored == seeded,
        "the wrapper should leave the recorded value in place, restored {restored:?} \
         with {seeded:?} recorded"
    );
    Ok(())
}

/// Run one documented example against a staged install and check the value it
/// stored.
fn assert_example_persists_the_directory(example: &DocumentedExample) -> Result<()> {
    let root = workspace(example)?;
    let install = install_directory(root.path())?;
    stage_binary(&install)?;
    let output = run_example(root.path(), example)?;
    assert_persisted(&output, &install, example)
}

/// Run one documented example without an installed executable and check that
/// it stopped before writing.
///
/// The existence guard is the only thing standing between a user who has not
/// installed Netsuke and a `PATH` entry pointing at nothing, so the failure
/// path is observed rather than inferred from the script's text.
fn assert_missing_executable_changes_nothing(example: &DocumentedExample) -> Result<()> {
    let root = workspace(example)?;
    let install = install_directory(root.path())?;
    let output = run_example(root.path(), example)?;
    assert_untouched(&output, example)?;
    ensure!(
        !output.status.success(),
        "{} should fail when {} holds no netsuke.exe, exited with {}",
        example.id,
        install.display(),
        output.status
    );
    Ok(())
}

/// Create a temporary workspace for one run of `example`.
fn workspace(example: &DocumentedExample) -> Result<tempfile::TempDir> {
    tempfile::tempdir().with_context(|| format!("create workspace for {}", example.id))
}

/// Create the directory the examples treat as the install root.
///
/// The examples read the program files directory from the environment and
/// append `netsuke` to it, so the staged install sits one level below the
/// temporary root the wrapper exports as every program files variable. Placing
/// it anywhere else would make the examples look at a directory this test never
/// populates.
///
/// The directory is returned because the assertions compare the stored value
/// against it.
fn install_directory(program_files_root: &Path) -> Result<PathBuf> {
    let install = program_files_root.join("netsuke");
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

/// Assert that the run stored the staged directory and kept the value type.
fn assert_persisted(output: &Output, install: &Path, example: &DocumentedExample) -> Result<()> {
    let stdout = require_success(output, example)?;
    let stored = marker(&stdout, "RESULT")
        .with_context(|| describe(output, example, "did not report RESULT"))?;
    let kind = marker(&stdout, "KIND")
        .with_context(|| describe(output, example, "did not report KIND"))?;
    let expected = format!("{SENTINEL_PATH};{}", install.display());
    ensure!(
        stored == expected,
        "{} should append {} to the raw user PATH, stored {stored:?}",
        example.id,
        install.display()
    );
    ensure!(
        kind == "ExpandString",
        "{} should preserve the PATH value type, stored {kind:?}",
        example.id
    );
    assert_restored(output, example)
}

/// Assert that a run which had to fail left the user `PATH` alone.
fn assert_untouched(output: &Output, example: &DocumentedExample) -> Result<()> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        marker(&stdout, "RESULT").is_none(),
        "{}",
        describe(
            output,
            example,
            "changed the user PATH despite the missing executable"
        )
    );
    let leftover = marker(&stdout, "LEFTOVER")
        .with_context(|| describe(output, example, "did not report LEFTOVER"))?;
    ensure!(
        leftover == SENTINEL_PATH,
        "{}",
        describe(
            output,
            example,
            &format!("should leave the user PATH at the sentinel, left {leftover:?}")
        )
    );
    assert_restored(output, example)
}

/// Return the wrapper's standard output, or the whole run as a failure.
fn require_success(output: &Output, example: &DocumentedExample) -> Result<String> {
    if !output.status.success() {
        bail!("{}", describe(output, example, "failed"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Assert that the wrapper put the original value back.
fn assert_restored(output: &Output, example: &DocumentedExample) -> Result<()> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let original = marker(&stdout, "ORIGINAL")
        .with_context(|| describe(output, example, "did not report ORIGINAL"))?;
    let restored = marker(&stdout, "RESTORED")
        .with_context(|| describe(output, example, "did not report RESTORED"))?;
    ensure!(
        restored == original,
        "{} wrapper should restore the original PATH, {restored:?} vs {original:?}",
        example.id
    );
    Ok(())
}

/// Describe a run in enough detail to diagnose it.
///
/// The wrapper's output is the only evidence of what the script did, so every
/// failure carries it rather than just the missing marker.
fn describe(output: &Output, example: &DocumentedExample, detail: &str) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!(
        "{} {detail}; wrapper exited with {}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        example.id, output.status
    )
}

/// Read the value of a `NAME=value` line the wrapper printed.
fn marker<'a>(stdout: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{name}=");
    stdout
        .lines()
        .find_map(|line| line.strip_prefix(prefix.as_str()))
}
