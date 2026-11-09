//! Run the isolated query and read back the traces it wrote.
//!
//! This is the fixture's half of the isolation case: it stages the verifiers the
//! sandbox script runs, hands the script its paths, and turns the trace files
//! back into strings. The restrictions themselves belong to
//! [`super::sandbox`], which owns the scripts and the capability probe.

use super::Fixture;
use super::InstalledKani;
use super::sandbox::{self, SandboxPaths};
use anyhow::{Context, Result};
use cap_std::ambient_authority;
use cap_std::fs::Dir;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::tempdir;

impl Fixture {
    /// Query the pinned version inside a namespace with no network and no
    /// writable storage.
    ///
    /// The sandbox builds a chroot whose every directory is a read-only bind,
    /// so the inherited home directory, temporary paths, and any cache are
    /// equally unwritable rather than merely outside a narrow allowlist. The
    /// query runs in a fresh network namespace and process namespace. The
    /// script reports each restriction it applied, and reports one deliberately
    /// writable control path, so the caller can prove the sandbox was effective
    /// rather than assume it.
    ///
    /// Three runs are traced: the real query, a control whose verifier attempts
    /// a suppressed write and a suppressed connection before reporting the
    /// pinned version, and a control recipe whose own lines attempt a suppressed
    /// write and a suppressed directory creation. The traces come back so the
    /// caller can require the first to be clean and the other two to name their
    /// attempts. When `installed` names a verifier, a fourth traced query runs
    /// it through the same recipe and its trace comes back too.
    ///
    /// # Errors
    ///
    /// Returns an error if the fake verifier, version file, sandbox root, trace
    /// directory, or sandbox script cannot be created, if `unshare` cannot be
    /// spawned, or if a trace cannot be read back.
    pub(crate) fn run_sandboxed_check(
        &self,
        repo: &Path,
        installed: Option<&InstalledKani>,
    ) -> Result<SandboxRun> {
        let (poison, marker) = self.poison_runner()?;
        let kani = self.fake_kani("#!/bin/sh\nprintf 'cargo-kani 0.67.0\\n'\n")?;
        let control = self.write_control_verifier()?;
        let dirty = self.write_dirty_recipe()?;
        let version_file = self.write_version_file()?;
        // Both directories must outlive the child; keep their handles in scope.
        let chroot = tempdir().context("create kani-check sandbox root")?;
        let trace = tempdir().context("create kani-check trace directory")?;
        let paths = SandboxPaths {
            repo,
            fixture: self.temp.path(),
            chroot: chroot.path(),
            kani: &kani,
            control: &control,
            version_file: &version_file,
            poison: &poison,
            trace: trace.path(),
            dirty: &dirty,
            installed,
        };
        let output = Command::new("unshare")
            .args([
                "--user",
                "--map-root-user",
                "--net",
                "--mount",
                "--pid",
                "--fork",
                "--",
                "sh",
                "-c",
            ])
            .arg(sandbox::script(&paths))
            .output()
            .context("run make kani-check inside an isolation namespace")?;
        let installed_trace = installed
            .map(|_| read_trace(trace.path(), "installed.strace"))
            .transpose()?;
        Ok(SandboxRun {
            query_trace: read_trace(trace.path(), "query.strace")?,
            control_trace: read_trace(trace.path(), "control.strace")?,
            dirty_trace: read_trace(trace.path(), "dirty.strace")?,
            installed_trace,
            output,
            marker,
        })
    }

    /// Write a verifier that attempts a suppressed write and connection.
    ///
    /// Both failures are discarded, as a caller that ignores them would, and
    /// the verifier then reports the pinned version so the query still succeeds.
    /// That is what makes the attempt invisible to the exit status and visible
    /// only to the trace.
    ///
    /// The control gets its own name rather than reusing the fixture's
    /// `fake_kani`: the two verifiers run in the same sandbox, and a shared name
    /// would leave the control in place of the real query.
    ///
    /// # Errors
    ///
    /// Returns an error if the script cannot be written or made executable.
    fn write_control_verifier(&self) -> Result<PathBuf> {
        let write_target = self.path("control-write");
        let script = format!(
            concat!(
                "#!/bin/sh\n",
                // The write runs in a subshell because `:` is a special
                // builtin: a failed redirection on one aborts the shell that
                // runs it, and only the subshell may pay that price.
                "( : > '{}' ) 2>/dev/null || true\n",
                "getent hosts example.com >/dev/null 2>&1 || true\n",
                "printf 'cargo-kani 0.67.0\\n'\n",
            ),
            write_target.display()
        );
        self.write_executable("control-kani", &script)?;
        Ok(self.path("control-kani"))
    }

    /// Write a recipe whose own lines attempt a suppressed write.
    ///
    /// The verifier control proves the reader fires on a verifier's attempts;
    /// this proves it fires on the recipe's. A recipe line that discards its
    /// failure leaves the make run looking exactly like one that never tried,
    /// so the trace is the only evidence, and the case requires this run to
    /// succeed. Both paths are on read-only mounts, so nothing is changed by
    /// running it.
    ///
    /// # Errors
    ///
    /// Returns an error if the makefile cannot be written.
    fn write_dirty_recipe(&self) -> Result<PathBuf> {
        let recipe = concat!(
            "# A recipe-shaped control: the lines below are what an accidental\n",
            "# write in the real recipe would look like, with each failure\n",
            "# discarded so the make run still succeeds.\n",
            ".PHONY: kani-check\n",
            "kani-check:\n",
            "\t@mkdir -p /usr/dirty-probe 2>/dev/null || true; \\\n",
            "\t( : > /etc/dirty-probe ) 2>/dev/null || true; \\\n",
            "\t$(KANI) --version >/dev/null 2>&1 || true; \\\n",
            "\tprintf 'Kani 0.67.0 matches.\\n'\n",
        );
        self.directory
            .write("dirty.mk", recipe)
            .context("write the recipe control")?;
        Ok(self.path("dirty.mk"))
    }
}

/// A completed isolation run and the traces it recorded.
pub(crate) struct SandboxRun {
    pub(crate) output: Output,
    pub(crate) marker: PathBuf,
    pub(crate) query_trace: String,
    pub(crate) control_trace: String,
    /// The trace of the control recipe whose lines attempt suppressed writes.
    pub(crate) dirty_trace: String,
    /// The trace of the installed verifier's query, when one was exercised.
    pub(crate) installed_trace: Option<String>,
}

/// Read one trace the sandboxed run wrote into the shared trace directory.
fn read_trace(directory: &Path, name: &str) -> Result<String> {
    let handle = Dir::open_ambient_dir(directory, ambient_authority())
        .with_context(|| format!("open the trace directory at {}", directory.display()))?;
    handle
        .read_to_string(name)
        .with_context(|| format!("read the trace at {}", directory.join(name).display()))
}
