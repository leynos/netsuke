//! Snapshot the human-readable cause of manifest, graph, and Ninja failures.
//!
//! Each case drives the real binary in a workspace whose `Netsukefile` is
//! relative, with a non-terminal stderr and a Ninja path that cannot exist, so
//! the failure is decided before any tool runs. The snapshots pin the wording
//! and layout a person sees by default; the semantic assertions pin the cause
//! each case exists to disclose, so a snapshot that churns cannot hide a lost
//! cause.

use super::create_netsuke_command;
use anyhow::{Context, Result, ensure};
use insta::Settings;
use rstest::rstest;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{TempDir, tempdir};
use test_support::fluent::normalize_fluent_isolates;

/// A manifest whose YAML flow sequence is never closed.
const UNCLOSED_BRACKET: &str = concat!("netsuke_version: \"1.0.0\"\n", "targets: [\n",);

/// A manifest naming a field that does not exist.
const UNKNOWN_FIELD: &str = concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: hello\n",
    "    comand: echo hello\n",
);

/// The rejected interpolation shown in the README.
///
/// Only the POSIX interpolation test reads it, so the constant shares that
/// test's gate and does not trip the Windows dead-code denial.
#[cfg(unix)]
const BACKTICK_INTERPOLATION: &str = concat!(
    "netsuke_version: '1.0.0'\n",
    "targets:\n",
    "  - name: output.txt\n",
    "    sources: input.txt\n",
    "    command: 'echo `cat {{ ins }}` > {{ outs }}'\n",
    "defaults: [output.txt]\n",
);

/// A script whose YAML double-quoted value contains a literal carriage return.
const CARRIAGE_RETURN_SCRIPT: &str = r#"netsuke_version: "1.0.0"
targets:
  - name: out.txt
    script: "echo \"a\rb\" > {{ outs }}"
defaults: [out.txt]
"#;

/// Two targets that require each other.
const CIRCULAR_DEPENDENCY: &str = concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: a.txt\n",
    "    sources: b.txt\n",
    "    command: \"cp {{ ins }} {{ outs }}\"\n",
    "  - name: b.txt\n",
    "    sources: a.txt\n",
    "    command: \"cp {{ ins }} {{ outs }}\"\n",
);

/// Write `manifest` as a relative `Netsukefile` in a fresh workspace.
fn workspace_with(manifest: &str) -> Result<TempDir> {
    let workspace = tempdir().context("create failure-snapshot workspace")?;
    fs::write(workspace.path().join("Netsukefile"), manifest)
        .context("write failure-snapshot manifest")?;
    Ok(workspace)
}

/// Run `generate` in `workspace` and return its normalized stderr.
///
/// `stdout` must stay empty on a failure, because the Ninja text of a failing
/// run is never a partial document. The runner's tracing preface carries a
/// timestamp, so it is removed rather than snapshotted; the diagnostic below it
/// is the output under test.
fn failing_generate(workspace: &Path) -> Result<String> {
    let output = create_netsuke_command(workspace)
        .env("NETSUKE_NINJA", "/nonexistent/ninja")
        .args([
            "--locale",
            "en-GB",
            "--color",
            "never",
            "--emoji",
            "never",
            "--progress",
            "never",
            "generate",
        ])
        .output()
        .context("run generate and capture its failure output")?;
    ensure!(
        !output.status.success(),
        "generate must fail for this case, got status {:?}",
        output.status
    );
    ensure!(
        output.stdout.is_empty(),
        "a failing generate must write nothing to stdout, got: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8(output.stderr).context("failure stderr should be UTF-8")?;
    Ok(normalize(workspace, &stderr))
}

/// Normalize localized text and redact the workspace path.
///
/// Only the temporary directory is redacted, and the file name survives it, so
/// a snapshot still shows which manifest was refused. Whitespace is preserved
/// because the diagnostic's layout is part of what the snapshot asserts.
fn normalize(workspace: &Path, stderr: &str) -> String {
    let without_log = normalize_fluent_isolates(stderr)
        .lines()
        .filter(|line| !line.contains("ERROR netsuke::human_diagnostics: runner failed"))
        .collect::<Vec<_>>()
        .join("\n");
    without_log.replace(&workspace.to_string_lossy().into_owned(), "<WORKSPACE>")
}

/// Snapshot `rendered` under the repository's snapshot directory.
fn assert_snapshot(name: &str, rendered: &str) {
    let mut settings = Settings::new();
    settings.set_snapshot_path(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots"));
    settings.bind(|| insta::assert_snapshot!(name, rendered));
}

/// Assert the required cause text, then snapshot the whole failure.
fn assert_failure_snapshot(name: &str, manifest: &str, expected: &[&str]) -> Result<()> {
    let workspace = workspace_with(manifest)?;
    let rendered = failing_generate(workspace.path())?;
    for text in expected {
        ensure!(
            rendered.contains(text),
            "{name} must report {text:?}, got: {rendered}"
        );
    }
    assert_snapshot(name, &rendered);
    Ok(())
}

/// Snapshot each portable failure and assert the cause it exists to disclose.
///
/// The YAML case also pins the line, column, and labelled snippet. The
/// structure and template cases pin that no snippet is drawn, and the graph
/// and Ninja cases pin the cause that the default output must name.
#[rstest]
#[case::yaml_syntax_failure(
    "yaml_syntax_failure",
    UNCLOSED_BRACKET,
    &[
        "netsuke::yaml::parse",
        "line 2, column 10",
        "Netsukefile:2:10",
        "invalid YAML",
    ],
)]
#[case::unknown_manifest_field(
    "unknown_manifest_field",
    UNKNOWN_FIELD,
    &["netsuke::manifest::structure", "unknown field `comand`"],
)]
#[case::undefined_template_variable(
    "undefined_template_variable",
    include_str!("../data/jinja_undefined.yml"),
    &["render target command", "undefined value (in <string>:1)"],
)]
#[case::rule_and_command_conflict(
    "rule_and_command_conflict",
    include_str!("../data/rule_command_conflict.yml"),
    &["netsuke::manifest::structure", "mutually exclusive"],
)]
#[case::missing_rule(
    "missing_rule",
    include_str!("../data/missing_rule.yml"),
    &[
        "Failed to build graph from the manifest.",
        "Rule 'missing' referenced by target 'hello.o' was not found.",
    ],
)]
#[case::duplicate_outputs(
    "duplicate_outputs",
    include_str!("../data/duplicate_outputs.yml"),
    &[
        "Failed to build graph from the manifest.",
        "Duplicate outputs detected",
        "hello.o",
    ],
)]
#[case::carriage_return_script(
    "carriage_return_script",
    CARRIAGE_RETURN_SCRIPT,
    &[
        "Failed to generate the Ninja manifest.",
        "unsafe Ninja control character",
    ],
)]
#[case::circular_dependency(
    "circular_dependency",
    CIRCULAR_DEPENDENCY,
    &["Circular dependency detected", "a.txt", "b.txt"],
)]
fn portable_failure_snapshot(
    #[case] name: &str,
    #[case] manifest: &str,
    #[case] expected: &[&str],
) -> Result<()> {
    assert_failure_snapshot(name, manifest, expected)
}

/// The README's rejected backtick command renders through the default output.
///
/// The rejection is decided only for POSIX and Bash command recipes, so the
/// Windows build has no equivalent failure to snapshot.
#[cfg(unix)]
#[test]
fn readme_backtick_interpolation_snapshot() -> Result<()> {
    assert_failure_snapshot(
        "readme_backtick_interpolation",
        BACKTICK_INTERPOLATION,
        &[
            "Failed to build graph from the manifest.",
            "Invalid command interpolation:",
        ],
    )
}

/// A missing manifest keeps the runner's code, summary, and recovery help.
#[test]
fn missing_manifest_snapshot() -> Result<()> {
    let workspace = tempdir().context("create missing-manifest workspace")?;
    let rendered = failing_generate(workspace.path())?;
    for text in [
        "netsuke::runner::manifest_not_found",
        "Manifest 'Netsukefile' not found",
        "Ensure the manifest exists or pass `--file` with the correct path.",
    ] {
        ensure!(
            rendered.contains(text),
            "the missing-manifest diagnostic must report {text:?}, got: {rendered}"
        );
    }
    assert_snapshot("missing_manifest", &rendered);
    Ok(())
}
