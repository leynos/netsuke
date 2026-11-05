//! Startup-boundary tests for trusted configured shell definitions.
//!
//! The authority tests prove which sources may declare `shells`. These tests
//! prove what startup does with the decision: a rejection stops the process
//! before a manifest is compiled, and a later `json = true` still selects JSON
//! diagnostics. The bounded-diagnostic tests live in [`diagnostics`], and the
//! acceptance tests that a trusted definition does not block a build live in
//! [`success`].

use anyhow::{Context, Result, ensure};
use assert_cmd::cargo::cargo_bin_cmd;
use rstest::rstest;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Output;
use tempfile::{TempDir, tempdir};
use test_support::fs as test_fs;

/// Name the file `generate` writes only when manifest compilation succeeds.
const SENTINEL: &str = "sentinel.ninja";

/// Render one definition whose fixed arguments never reach a diagnostic.
fn definition_body(name: &str) -> String {
    format!(
        "[[shells]]\nname = \"{name}\"\nexecutable = \"{name}-bin\"\nargs = [\"--tag\", \"{name}\"]\n"
    )
}

/// An isolated workspace with a harmless manifest and no host configuration.
struct Startup {
    root: TempDir,
}

impl Startup {
    /// Create the workspace and copy the harmless manifest into it.
    fn new() -> Result<Self> {
        let root = tempdir().context("create temporary workspace")?;
        test_fs::copy("tests/data/minimal.yml", root.path().join("Netsukefile"))
            .context("copy harmless manifest")?;
        Ok(Self { root })
    }

    /// Borrow the workspace root.
    fn path(&self) -> &Path {
        self.root.path()
    }

    /// Write `contents` to `relative` beneath the workspace.
    fn write(&self, relative: &str, contents: &str) -> Result<PathBuf> {
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
    fn command(&self, args: &[&str]) -> assert_cmd::Command {
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
    fn generate(&self) -> Result<Output> {
        self.command(&["generate", "--output", SENTINEL])
            .output()
            .context("run netsuke generate")
    }

    /// Report whether the sentinel exists, which means compilation ran.
    fn sentinel_exists(&self) -> bool {
        self.path().join(SENTINEL).exists()
    }
}

/// Decode a child's standard error as text.
fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Assert a run failed, named `key`, and compiled no manifest.
fn ensure_rejected_before_compilation(startup: &Startup, output: &Output, key: &str) -> Result<()> {
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
fn json_failure(output: &Output) -> Result<Value> {
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
/// The two formats snapshotted below are the ones a user or a machine consumer
/// observes, so they are the only outputs pinned with `insta`; exact
/// collections and typed outcomes use ordinary assertions. A temporary
/// workspace path, a tracing timestamp, and the crate version would otherwise
/// make an unchanged diagnostic look like a drift.
fn normalised(stderr: &str, startup: &Startup) -> Result<String> {
    let path = startup.path().to_string_lossy().into_owned();
    let located = stderr.replace(&path, "[WORKSPACE]");
    let versioned = located.replace(env!("CARGO_PKG_VERSION"), "[VERSION]");
    let timestamp =
        regex::Regex::new(TRACE_TIMESTAMP).context("compile the tracing timestamp pattern")?;
    Ok(timestamp
        .replace_all(&versioned, "[TIMESTAMP] ")
        .into_owned())
}

#[test]
fn text_diagnostic_format_is_stable() -> Result<()> {
    let startup = Startup::new()?;
    startup.write(
        "user/netsuke/config.toml",
        "[[shells]]\nname = \"dash\"\nexecutable = \"bin/dash\"\nargs = [\"-c\"]\n",
    )?;

    let output = startup.generate()?;

    ensure!(
        !output.status.success(),
        "startup should reject the definition"
    );
    let rendered = normalised(&stderr_of(&output), &startup)?;
    insta::assert_snapshot!("shells_text_diagnostic", rendered);
    Ok(())
}

#[test]
fn json_diagnostic_format_is_stable() -> Result<()> {
    let startup = Startup::new()?;
    startup.write(
        "user/netsuke/config.toml",
        "json = true\n[[shells]]\nname = \"dash\"\nexecutable = \"bin/dash\"\nargs = [\"-c\"]\n",
    )?;

    let output = startup.generate()?;

    json_failure(&output)?;
    let rendered = normalised(&stderr_of(&output), &startup)?;
    insta::assert_snapshot!("shells_json_diagnostic", rendered);
    Ok(())
}

#[test]
fn project_definition_fails_before_manifest_compilation() -> Result<()> {
    let startup = Startup::new()?;
    startup.write(".netsuke.toml", &definition_body("dash"))?;

    let output = startup.generate()?;

    ensure_rejected_before_compilation(&startup, &output, "shells")?;
    ensure!(
        stderr_of(&output).contains("operator"),
        "failure should state the operator-authority rule, got: {}",
        stderr_of(&output)
    );
    Ok(())
}

#[test]
fn explicit_selector_does_not_grant_authority() -> Result<()> {
    let startup = Startup::new()?;
    let untrusted = startup.write("untrusted.toml", &definition_body("custom"))?;
    let selector = untrusted.to_string_lossy().into_owned();

    let output = startup
        .command(&["--config", &selector, "generate", "--output", SENTINEL])
        .output()
        .context("run netsuke with an explicit selector")?;

    ensure_rejected_before_compilation(&startup, &output, "shells")
}

#[test]
fn environment_selector_does_not_grant_authority() -> Result<()> {
    let startup = Startup::new()?;
    let untrusted = startup.write("untrusted.toml", &definition_body("custom"))?;

    let output = startup
        .command(&["generate", "--output", SENTINEL])
        .env("NETSUKE_CONFIG", &untrusted)
        .output()
        .context("run netsuke with an environment selector")?;

    ensure_rejected_before_compilation(&startup, &output, "shells")
}

#[test]
fn inherited_project_definition_is_rejected() -> Result<()> {
    let startup = Startup::new()?;
    startup.write("base.toml", &definition_body("dash"))?;
    startup.write(".netsuke.toml", "extends = \"base.toml\"\n")?;

    let output = startup.generate()?;

    ensure_rejected_before_compilation(&startup, &output, "shells")
}

#[test]
fn malformed_trusted_definition_names_its_field() -> Result<()> {
    let startup = Startup::new()?;
    startup.write(
        "user/netsuke/config.toml",
        "[[shells]]\nname = \"dash\"\nexecutable = \"bin/dash\"\nargs = [\"-c\"]\n",
    )?;

    let output = startup.generate()?;

    ensure_rejected_before_compilation(&startup, &output, "shells[0].executable")?;
    ensure!(
        stderr_of(&output).contains("absolute"),
        "failure should explain the absolute-path rule, got: {}",
        stderr_of(&output)
    );
    Ok(())
}

#[test]
fn duplicate_trusted_definitions_are_rejected() -> Result<()> {
    let startup = Startup::new()?;
    startup.write(
        "user/netsuke/config.toml",
        &format!("{}{}", definition_body("dash"), definition_body("dash")),
    )?;

    let output = startup.generate()?;

    ensure_rejected_before_compilation(&startup, &output, "shells")?;
    let stderr = stderr_of(&output);
    ensure!(
        stderr.contains("shells[1]") && stderr.contains("shells[0]"),
        "failure should name both duplicate positions, got: {stderr}"
    );
    ensure!(
        stderr.contains("already names"),
        "failure should explain the collision, got: {stderr}"
    );
    Ok(())
}

#[test]
fn later_json_preference_is_honoured_after_rejection() -> Result<()> {
    let startup = Startup::new()?;
    startup.write("base.toml", &definition_body("dash"))?;
    startup.write(".netsuke.toml", "extends = \"base.toml\"\njson = true\n")?;

    let output = startup.generate()?;

    json_failure(&output)?;
    ensure!(
        stderr_of(&output).contains("shells"),
        "JSON failure should name the rejected key, got: {}",
        stderr_of(&output)
    );
    ensure!(
        !startup.sentinel_exists(),
        "startup should stop before manifest compilation"
    );
    Ok(())
}

#[test]
fn malformed_operator_definition_honours_json_preference() -> Result<()> {
    let startup = Startup::new()?;
    startup.write(
        "user/netsuke/config.toml",
        "json = true\n[[shells]]\nname = \"dash\"\nexecutable = \"bin/dash\"\nargs = [\"-c\"]\n",
    )?;

    let output = startup.generate()?;

    json_failure(&output)?;
    ensure!(
        stderr_of(&output).contains("shells[0].executable"),
        "JSON failure should name the rejected field, got: {}",
        stderr_of(&output)
    );
    Ok(())
}

#[rstest]
#[case::flat("NETSUKE_SHELLS")]
#[case::nested("NETSUKE_SHELLS__0__NAME")]
#[case::nested_arguments("NETSUKE_SHELLS__0__ARGS")]
#[case::mixed_case("netsuke_Shells")]
fn environment_aliases_are_denied(#[case] variable: &str) -> Result<()> {
    let startup = Startup::new()?;

    let output = startup
        .command(&["generate", "--output", SENTINEL])
        .env(variable, "dash")
        .output()
        .with_context(|| format!("run netsuke with {variable} set"))?;

    ensure!(!output.status.success(), "startup should reject {variable}");
    let stderr = stderr_of(&output);
    ensure!(
        stderr.contains("configuration file"),
        "{variable} should be rejected as file-only, got: {stderr}"
    );
    ensure!(
        !startup.sentinel_exists(),
        "{variable} should stop startup before manifest compilation"
    );
    Ok(())
}

mod diagnostics;
mod success;
