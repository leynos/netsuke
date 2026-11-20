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
    run_example, run_example_interrupted, run_example_leaving_spent_backup, wrapper::SENTINEL_PATH,
};

/// Verify the whole documented `PATH` contract: each script appends its
/// directory and keeps the value expandable, changes nothing when the
/// executable is absent, and a run killed after appending is repaired by the
/// next process.
///
/// Every case runs inside one test on purpose. Each run rewrites the same user
/// `PATH` value, and the recovery case leaves a crashed state behind between
/// two runs, so separate test functions could interleave and either read each
/// other's value or repair the crashed state before its second run sees it.
#[test]
#[cfg_attr(
    not(windows),
    ignore = "the documented scripts write the Windows user environment key"
)]
fn documented_path_setup_persists_the_directory() -> Result<()> {
    // Recovery runs first so a crashed run left behind by an earlier suite is
    // repaired before the assertions that expect a settled value.
    assert_interrupted_run_is_recovered()?;
    assert_spent_backup_is_cleaned_up()?;
    for example_id in ["guide-windows-path", "guide-windows-path-beta4"] {
        let example = documented_example(example_id)?;
        assert_example_persists_the_directory(&example)?;
        assert_missing_executable_changes_nothing(&example)?;
    }
    Ok(())
}

/// Verify that a run killed after the example appends its directory is
/// repaired by a separate process.
///
/// The recovery exists for exactly this crash. Without it the next run adopts
/// the sentinel-plus-directory value as the user's own original, and the
/// entries the backup recorded are lost for good. The first process really
/// ends without restoring anything, so the second one meets the registry, the
/// backup and the abandoned mutex that a crash leaves, rather than a state
/// handed to it inside one process.
///
/// If this fails after the first process, the backup is still in place and the
/// next run of the suite restores the user's `PATH` from it.
fn assert_interrupted_run_is_recovered() -> Result<()> {
    let root = tempfile::tempdir().context("create the recovery workspace")?;
    let install = install_directory(root.path())?;
    let example = DocumentedExample {
        id: "recovery-probe".to_owned(),
        language: "powershell".to_owned(),
        body: "# The recovery under test runs before this script.\n".to_owned(),
    };
    let interrupted = run_example_interrupted(root.path(), &example)?;
    // The first process may have left the staged directory in the user's PATH,
    // so the recovery run below must happen whatever these checks find. They
    // are recorded here and reported only after recovery has repaired the
    // value; failing early would skip the repair and leave the registry
    // modified for every later run.
    let first_run_problem = interrupted_run_problem(&interrupted, &install, &example);
    let interrupted_recorded = marker(&String::from_utf8_lossy(&interrupted.stdout), "ORIGINAL")
        .map(str::to_owned)
        .with_context(|| {
            describe(
                &interrupted,
                &example,
                "did not report the value it recorded",
            )
        });

    let recovered = run_example(root.path(), &example)?;
    // Only now that the repair process has run is it safe to report what the
    // interrupted half got wrong.
    if let Some(problem) = first_run_problem {
        bail!("{problem}");
    }
    let recorded = interrupted_recorded?;
    let stdout = require_success(&recovered, &example)?;
    let captured = marker(&stdout, "ORIGINAL").with_context(|| {
        describe(
            &recovered,
            &example,
            "did not report ORIGINAL after recovery",
        )
    })?;
    ensure!(
        captured == recorded,
        "the next run should restore the value the interrupted run recorded, captured \
         {captured:?} with {recorded:?} recorded"
    );
    ensure!(
        !captured.starts_with(SENTINEL_PATH),
        "the recovered value should not carry the sentinel, captured {captured:?}"
    );
    let restored = marker(&stdout, "RESTORED")
        .with_context(|| describe(&recovered, &example, "did not report RESTORED"))?;
    ensure!(
        restored == recorded,
        "the run should leave the recovered value in place, restored {restored:?} with \
         {recorded:?} recorded"
    );
    Ok(())
}

/// Describe what the interrupted run got wrong, if anything.
///
/// The check is returned rather than asserted so the caller can run the
/// recovery process first: the run under test leaves the registry modified by
/// design, and an early `ensure!` would skip the only repair. `None` means the
/// interrupted half behaved as the crash it simulates.
fn interrupted_run_problem(
    interrupted: &Output,
    install: &Path,
    example: &DocumentedExample,
) -> Option<String> {
    if interrupted.status.success() {
        return Some(describe(
            interrupted,
            example,
            "should end without restoring the value it wrote",
        ));
    }
    let stdout = String::from_utf8_lossy(&interrupted.stdout).into_owned();
    let install_text = install.display().to_string();
    match marker(&stdout, "INTERRUPTED") {
        None => Some(describe(
            interrupted,
            example,
            "did not report the directory it left in PATH",
        )),
        Some(appended) if appended != install_text => Some(format!(
            "the interrupted run should leave the staged directory {install_text} in PATH, \
             left {appended:?}"
        )),
        Some(_) => None,
    }
}

/// Verify that a backup a completed restore left behind is removed rather than
/// refused.
///
/// A run killed between its restore and the delete of its backup leaves the
/// registry holding the original value and the backup still on disk. Refusing
/// that state would make every later run fail against a backup it has already
/// finished restoring, so the next run must recognise it, remove the spent
/// file and carry on with the documented setup.
///
/// If this fails after the first process, the backup is still in place and the
/// next run of the suite removes it.
fn assert_spent_backup_is_cleaned_up() -> Result<()> {
    let root = tempfile::tempdir().context("create the spent-backup workspace")?;
    let example = DocumentedExample {
        id: "spent-backup-probe".to_owned(),
        language: "powershell".to_owned(),
        body: "# The recovery under test runs before this script.\n".to_owned(),
    };
    let crashed = run_example_leaving_spent_backup(root.path(), &example)?;
    ensure!(
        !crashed.status.success(),
        "{}",
        describe(
            &crashed,
            &example,
            "should end after restoring without deleting the backup"
        )
    );
    let crashed_stdout = String::from_utf8_lossy(&crashed.stdout).into_owned();
    let spent = marker(&crashed_stdout, "SPENT")
        .with_context(|| describe(&crashed, &example, "did not report the value it restored"))?;
    let recorded = marker(&crashed_stdout, "ORIGINAL")
        .with_context(|| describe(&crashed, &example, "did not report the value it recorded"))?;
    ensure!(
        spent == recorded,
        "the spent backup should describe the restored value, SPENT={spent:?} with \
         ORIGINAL={recorded:?}"
    );

    let recovered = run_example(root.path(), &example)?;
    let stdout = require_success(&recovered, &example)?;
    let captured = marker(&stdout, "ORIGINAL").with_context(|| {
        describe(
            &recovered,
            &example,
            "did not report ORIGINAL after clearing the spent backup",
        )
    })?;
    ensure!(
        captured == spent,
        "the run should keep the restored value while clearing the spent backup, captured \
         {captured:?} with {spent:?} restored earlier"
    );
    let restored = marker(&stdout, "RESTORED")
        .with_context(|| describe(&recovered, &example, "did not report RESTORED"))?;
    ensure!(
        restored == spent,
        "the run should leave the restored value in place, restored {restored:?} with \
         {spent:?} restored earlier"
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
