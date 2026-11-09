//! Workspace fixture and assertion helpers for the startup-boundary tests.
//!
//! Each scenario needs the same three things: a harmless manifest in an
//! isolated workspace, a child process whose configuration roots all point
//! inside that workspace, and a way to say what a rejection must look like.
//! Keeping them here lets the parent module and its sibling test modules state
//! only the behaviour under test.

use anyhow::{Context, Result, ensure};
use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Output;
use tempfile::{TempDir, tempdir};
use test_support::fs as test_fs;

/// Name the file `generate` writes only when manifest compilation succeeds.
pub(super) const SENTINEL: &str = "sentinel.ninja";

/// The operator configuration path relative to a workspace root.
pub(super) const OPERATOR_CONFIG: &str = "user/netsuke/config.toml";

/// A definition whose executable is relative, so validation must reject it.
pub(super) const RELATIVE_EXECUTABLE: &str =
    "[[shells]]\nname = \"dash\"\nexecutable = \"bin/dash\"\nargs = [\"-c\"]\n";

/// Render one definition whose fixed arguments never reach a diagnostic.
pub(super) fn definition_body(name: &str) -> String {
    format!(
        "[[shells]]\nname = \"{name}\"\nexecutable = \"{name}-bin\"\nargs = [\"--tag\", \"{name}\"]\n"
    )
}

/// An isolated workspace with a harmless manifest and no host configuration.
pub(super) struct Startup {
    root: TempDir,
}

impl Startup {
    /// Create the workspace and copy the harmless manifest into it.
    pub(super) fn new() -> Result<Self> {
        let root = tempdir().context("create temporary workspace")?;
        test_fs::copy("tests/data/minimal.yml", root.path().join("Netsukefile"))
            .context("copy harmless manifest")?;
        Ok(Self { root })
    }

    /// Borrow the workspace root.
    pub(super) fn path(&self) -> &Path {
        self.root.path()
    }

    /// Write `contents` to `relative` beneath the workspace.
    pub(super) fn write(&self, relative: &str, contents: &str) -> Result<PathBuf> {
        let path = self.path().join(relative);
        if let Some(parent) = path.parent() {
            test_fs::create_dir_all(parent)
                .with_context(|| format!("create directory {}", parent.display()))?;
        }
        test_fs::write(&path, contents).with_context(|| format!("write {}", path.display()))?;
        Ok(path)
    }

    /// Build a command whose configuration roots all point inside the workspace.
    ///
    /// `HOME` names a `home` subdirectory rather than the workspace root, so
    /// the project file at the root is never also reachable as a home root.
    pub(super) fn command(&self, args: &[&str]) -> assert_cmd::Command {
        let mut command = cargo_bin_cmd!("netsuke");
        command
            .current_dir(self.path())
            .env_clear()
            .env("HOME", self.path().join("home"))
            .env("XDG_CONFIG_HOME", self.path().join("user"))
            .env("XDG_CONFIG_DIRS", self.path().join("system"))
            .args(args);
        command
    }

    /// Run `generate` into the sentinel file.
    pub(super) fn generate(&self) -> Result<Output> {
        self.command(&["generate", "--output", SENTINEL])
            .output()
            .context("run netsuke generate")
    }

    /// Report whether the sentinel exists, which means compilation ran.
    pub(super) fn sentinel_exists(&self) -> bool {
        self.path().join(SENTINEL).exists()
    }
}

/// Write `body` as the operator configuration file and run `generate`.
///
/// `body` is the whole file, so a caller that also sets `json = true` can put
/// that preference ahead of the definition in the same layer.
pub(super) fn generate_with_operator_config(startup: &Startup, body: &str) -> Result<Output> {
    startup.write(OPERATOR_CONFIG, body)?;
    startup.generate()
}

/// Decode a child's standard error as text.
pub(super) fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Assert a run failed, named `key`, and compiled no manifest.
pub(super) fn ensure_rejected_before_compilation(
    startup: &Startup,
    output: &Output,
    key: &str,
) -> Result<()> {
    ensure!(
        !output.status.success(),
        "expected startup to fail, but it succeeded with: {}",
        stderr_of(output)
    );
    let stderr = stderr_of(output);
    ensure!(
        stderr.contains("Validation failed for") && stderr.contains(key),
        "failure should report a validation fault for {key:?}, got: {stderr}"
    );
    ensure!(
        !startup.sentinel_exists(),
        "startup should stop before manifest compilation, but {} exists",
        startup.path().join(SENTINEL).display()
    );
    Ok(())
}

/// Assert the child failed with exactly one parseable JSON diagnostic document.
pub(super) fn json_failure(output: &Output) -> Result<Value> {
    ensure!(!output.status.success(), "startup should still fail");
    let stderr = stderr_of(output);
    let document: Value = serde_json::from_str(stderr.trim())
        .with_context(|| format!("failure should be one JSON document, got: {stderr}"))?;
    ensure!(
        document.get("diagnostics").is_some(),
        "JSON failure should carry diagnostics, got: {stderr}"
    );
    Ok(document)
}

/// Match the RFC 3339 prefix a tracing line stamps onto each record.
const TRACE_TIMESTAMP: &str = r"(?m)^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z ";

/// Replace run-specific prefixes so a snapshot survives the host and the clock.
///
/// The two formats snapshotted by the parent module are the ones a user or a
/// machine consumer observes, so they are the only outputs pinned with `insta`;
/// exact collections and typed outcomes use ordinary assertions. A temporary
/// workspace path, a tracing timestamp, and the crate version would otherwise
/// make an unchanged diagnostic look like a drift.
pub(super) fn normalised(stderr: &str, startup: &Startup) -> Result<String> {
    let path = startup.path().to_string_lossy().into_owned();
    let located = stderr.replace(&path, "[WORKSPACE]");
    let versioned = located.replace(env!("CARGO_PKG_VERSION"), "[VERSION]");
    let timestamp =
        regex::Regex::new(TRACE_TIMESTAMP).context("compile the tracing timestamp pattern")?;
    Ok(timestamp
        .replace_all(&versioned, "[TIMESTAMP] ")
        .into_owned())
}
