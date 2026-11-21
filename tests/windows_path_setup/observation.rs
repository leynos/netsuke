//! Read the report a wrapper run writes and check the contract it states.
//!
//! The wrapper is the only witness to what a documented fence did: it reports
//! the user `PATH` value it recorded, the value the example left behind,
//! whether the example changed the environment key's value, the child process
//! `PATH` before and after the example, and the value it restored. Parsing
//! that report once here keeps the behavioural test reading as the contract
//! rather than as marker handling, and gives every failure the whole report as
//! its context.

use std::{path::Path, process::Output};

use anyhow::{Context, Result, bail, ensure};

use super::wrapper::SENTINEL_PATH;
use crate::documentation_examples::DocumentedExample;

/// Read the value of a `NAME=value` line the wrapper printed.
#[must_use]
pub fn marker<'a>(stdout: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{name}=");
    stdout
        .lines()
        .find_map(|line| line.strip_prefix(prefix.as_str()))
}

/// Describe a run in enough detail to diagnose it.
///
/// The wrapper's output is the only evidence of what the script did, so every
/// failure carries it rather than just the missing marker.
#[must_use]
pub fn describe(output: &Output, example: &DocumentedExample, detail: &str) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!(
        "{} {detail}; wrapper exited with {}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        example.id, output.status
    )
}

/// Return the wrapper's standard output, or the whole run as a failure.
///
/// # Errors
///
/// Returns an error carrying the run's output when the wrapper exited
/// unsuccessfully.
pub fn require_success(output: &Output, example: &DocumentedExample) -> Result<String> {
    if !output.status.success() {
        bail!("{}", describe(output, example, "failed"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The observations one wrapper run reported, parsed from its standard output.
///
/// Every documented fence starts from [`SENTINEL_PATH`], so the values here are
/// compared against that sentinel and against the staged install directory
/// rather than against whatever the host's user `PATH` happens to hold.
pub struct RunReport<'a> {
    /// The run the report came from.
    output: &'a Output,
    /// The example the run executed, used in diagnostics.
    example: &'a DocumentedExample,
    /// The user `PATH` value the wrapper recorded before the example ran.
    original: String,
    /// The value the wrapper restored after the example finished.
    restored: String,
    /// The value the example left behind, read before the wrapper's restore.
    leftover: String,
    /// The value the example stored, if it reached its own write.
    result: Option<String>,
    /// The value type the example stored, if it reached its own write.
    kind: Option<String>,
    /// The child process `PATH` the example started from.
    process_before: String,
    /// The child process `PATH` the example left behind.
    process_after: String,
    /// Whether the example changed the environment key's value at all.
    key_changed: bool,
}

impl<'a> RunReport<'a> {
    /// Parse the report of one wrapper run.
    ///
    /// The markers read here are written whatever the example does, so a run
    /// whose example threw still produces a report. `RESULT` and `KIND` are the
    /// exception: the wrapper writes them only after the example returns, so
    /// their absence is itself an observation.
    ///
    /// # Errors
    ///
    /// Returns an error naming the missing or unreadable marker and quoting the
    /// run's output when the wrapper did not report it.
    pub fn read(output: &'a Output, example: &'a DocumentedExample) -> Result<Self> {
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let required = |name: &str| {
            marker(&stdout, name)
                .map(str::to_owned)
                .with_context(|| describe(output, example, &format!("did not report {name}")))
        };
        let key_changed = match required("KEYCHANGED")?.to_ascii_lowercase().as_str() {
            "true" => true,
            "false" => false,
            other => bail!(
                "{}",
                describe(
                    output,
                    example,
                    &format!("reported KEYCHANGED={other}, which is not a boolean")
                )
            ),
        };
        Ok(Self {
            output,
            example,
            original: required("ORIGINAL")?,
            restored: required("RESTORED")?,
            leftover: required("LEFTOVER")?,
            result: marker(&stdout, "RESULT").map(str::to_owned),
            kind: marker(&stdout, "KIND").map(str::to_owned),
            process_before: required("PROCESS_BEFORE")?,
            process_after: required("PROCESS_AFTER")?,
            key_changed,
        })
    }

    /// Return the report only when the wrapper exited successfully.
    ///
    /// # Errors
    ///
    /// Returns an error carrying the run's output when the wrapper failed.
    pub fn require_success(self) -> Result<Self> {
        if self.output.status.success() {
            Ok(self)
        } else {
            bail!("{}", describe(self.output, self.example, "failed"))
        }
    }

    /// Assert that the run stored the staged directory in both `PATH` values.
    ///
    /// The user value must gain the directory and keep its expandable type,
    /// and the process value must gain the same directory for the rest of that
    /// shell: both documented fences promise the second so that
    /// `Get-Command` finds the executable in the same session.
    ///
    /// # Errors
    ///
    /// Returns an error naming the observation that disagrees with the
    /// documented contract.
    pub fn assert_persisted(&self, install: &Path) -> Result<()> {
        let expected = format!("{SENTINEL_PATH};{}", install.display());
        let stored = self
            .result
            .as_deref()
            .with_context(|| describe(self.output, self.example, "did not report RESULT"))?;
        ensure!(
            stored == expected,
            "{} should append {} to the raw user PATH, stored {stored:?}",
            self.example.id,
            install.display()
        );
        let kind = self
            .kind
            .as_deref()
            .with_context(|| describe(self.output, self.example, "did not report KIND"))?;
        ensure!(
            kind == "ExpandString",
            "{} should preserve the PATH value type, stored {kind:?}",
            self.example.id
        );
        ensure!(
            self.key_changed,
            "{}",
            describe(
                self.output,
                self.example,
                "stored the directory without the environment key recording a change"
            )
        );
        let expected_process = format!("{};{}", self.process_before, install.display());
        ensure!(
            self.process_after == expected_process,
            "{} should append {} to the process PATH, which ran from {:?} to {:?} \
             instead of {expected_process:?}",
            self.example.id,
            install.display(),
            self.process_before,
            self.process_after
        );
        self.assert_restored()
    }

    /// Assert that a run which had to fail changed nothing at all.
    ///
    /// The existence guard is the only thing standing between a user who has
    /// not installed Netsuke and a `PATH` entry pointing at nothing, so the
    /// failure is observed on every surface it could reach: the user value, the
    /// fact that the key's value never changed, the process `PATH`, and the exit
    /// status. Checking only the stored value would accept a run that changed it
    /// and changed it back.
    ///
    /// # Errors
    ///
    /// Returns an error naming the observation that shows the run got past the
    /// guard.
    pub fn assert_untouched(&self, install: &Path) -> Result<()> {
        ensure!(
            !self.output.status.success(),
            "{} should fail when {} holds no netsuke.exe, exited with {}",
            self.example.id,
            install.display(),
            self.output.status
        );
        ensure!(
            self.result.is_none(),
            "{}",
            describe(
                self.output,
                self.example,
                "changed the user PATH despite the missing executable"
            )
        );
        ensure!(
            self.leftover == SENTINEL_PATH,
            "{}",
            describe(
                self.output,
                self.example,
                &format!(
                    "should leave the user PATH at the sentinel, left {:?}",
                    self.leftover
                )
            )
        );
        ensure!(
            !self.key_changed,
            "{}",
            describe(
                self.output,
                self.example,
                "changed the environment key even though the executable is missing"
            )
        );
        ensure!(
            self.process_after == self.process_before,
            "{}",
            describe(
                self.output,
                self.example,
                &format!(
                    "changed the process PATH from {:?} to {:?} despite the missing executable",
                    self.process_before, self.process_after
                )
            )
        );
        self.assert_restored()
    }

    /// Assert that the wrapper put the original value back.
    ///
    /// # Errors
    ///
    /// Returns an error when the restored value differs from the recorded one.
    pub fn assert_restored(&self) -> Result<()> {
        ensure!(
            self.restored == self.original,
            "{} wrapper should restore the original PATH, {:?} vs {:?}",
            self.example.id,
            self.restored,
            self.original
        );
        Ok(())
    }
}
