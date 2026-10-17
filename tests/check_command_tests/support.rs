//! Shared fixtures and JSON helpers for `netsuke check` integration tests.

use anyhow::{Context, Result};
use rstest::fixture;
use serde_json::Value;
use tempfile::TempDir;
use test_support::fs as test_fs;
use test_support::netsuke::{NetsukeRun, run_netsuke_in};

/// A manifest whose only finding is a warning.
pub(super) const WARNS: &str = concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: out.txt\n",
    "    command: \"cp $$SRC {{ outs }}\"\n",
);

/// A manifest that trips no rule.
pub(super) const CLEAN: &str = concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: out.txt\n",
    "    command: \"touch {{ outs }}\"\n",
);

/// A workspace holding a manifest, kept alive for the duration of a test.
pub(super) struct Workspace {
    /// The temporary directory backing the workspace.
    pub(super) directory: TempDir,
}

impl Workspace {
    /// Create a workspace whose `Netsukefile` holds `manifest`.
    pub(super) fn new(manifest: &str) -> Result<Self> {
        let directory = TempDir::new().context("create a workspace")?;
        test_fs::write(directory.path().join("Netsukefile"), manifest)
            .context("write the manifest")?;
        Ok(Self { directory })
    }

    /// Run `netsuke` with `args` in this workspace.
    pub(super) fn run(&self, args: &[&str]) -> Result<NetsukeRun> {
        run_netsuke_in(self.directory.path(), args)
    }
}

/// A workspace whose manifest reports one warning.
#[fixture]
pub(super) fn warning_workspace() -> Result<Workspace> {
    Workspace::new(WARNS)
}

/// A workspace whose manifest reports nothing.
#[fixture]
pub(super) fn clean_workspace() -> Result<Workspace> {
    Workspace::new(CLEAN)
}

/// Parse a run's stdout as the JSON document it should have written.
pub(super) fn document(run: &NetsukeRun) -> Result<Value> {
    serde_json::from_str(&run.stdout).context("parse the JSON document on stdout")
}

/// Parse a run's stderr as the JSON diagnostic document it should have written.
pub(super) fn diagnostic(run: &NetsukeRun) -> Result<Value> {
    serde_json::from_str(&run.stderr).context("parse the JSON document on stderr")
}

/// Write `config` beside the workspace manifest and return its path argument.
pub(super) fn write_config(workspace: &Workspace, config: &str) -> Result<String> {
    let path = workspace.directory.path().join("netsuke.toml");
    test_fs::write(&path, config).context("write the check configuration")?;
    path.to_str()
        .map(str::to_owned)
        .context("temporary config path should be UTF-8")
}
