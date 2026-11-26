//! Snapshot the human-readable cause of manifest, graph, and Ninja failures.
//!
//! Each case drives the real binary in a workspace whose `Netsukefile` is
//! relative, with a non-terminal stderr and a Ninja path that cannot exist, so
//! the failure is decided before any tool runs. The snapshots pin the wording
//! and layout a person sees by default; the semantic assertions pin the cause
//! each case exists to disclose, so a snapshot that churns cannot hide a lost
//! cause. The README backtick rejection and the carriage-return script are
//! decided only on the POSIX shell path, so those two cases carry a platform
//! gate instead of asserting one outcome for both transports.

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
///
/// Only the POSIX shell path rejects it, so the constant shares that test's
/// gate and does not trip the Windows dead-code denial.
#[cfg(not(windows))]
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

/// One failure-snapshot case: the manifest to run and the cause it must show.
struct FailureCase<'a> {
    /// Snapshot name, also the case label reported on failure.
    name: &'a str,
    /// Manifest written as the workspace's relative `Netsukefile`.
    manifest: &'a str,
    /// Cause fragments the rendered failure must name.
    expected: &'a [&'a str],
}

/// Assert the required cause text, then snapshot the whole failure.
fn assert_failure_snapshot(case: &FailureCase<'_>) -> Result<()> {
    let workspace = workspace_with(case.manifest)?;
    let rendered = failing_generate(workspace.path())?;
    for text in case.expected {
        ensure!(
            rendered.contains(text),
            "{} must report {text:?}, got: {rendered}",
            case.name
        );
    }
    assert_snapshot(case.name, &rendered);
    Ok(())
}

/// Snapshot each portable failure and assert the cause it exists to disclose.
///
/// The YAML case also pins the line, column, and labelled snippet. The
/// structure and template cases pin that no snippet is drawn, and the graph
/// cases pin the cause that the default output must name.
#[rstest]
#[case::yaml_syntax_failure(FailureCase {
    name: "yaml_syntax_failure",
    manifest: UNCLOSED_BRACKET,
    expected: &[
        "netsuke::yaml::parse",
        "line 2, column 10",
        "Netsukefile:2:10",
        "invalid YAML",
    ],
})]
#[case::unknown_manifest_field(FailureCase {
    name: "unknown_manifest_field",
    manifest: UNKNOWN_FIELD,
    expected: &["netsuke::manifest::structure", "unknown field `comand`"],
})]
#[case::undefined_template_variable(FailureCase {
    name: "undefined_template_variable",
    manifest: include_str!("../data/jinja_undefined.yml"),
    expected: &["render target command", "undefined value (in <string>:1)"],
})]
#[case::rule_and_command_conflict(FailureCase {
    name: "rule_and_command_conflict",
    manifest: include_str!("../data/rule_command_conflict.yml"),
    expected: &["netsuke::manifest::structure", "mutually exclusive"],
})]
#[case::missing_rule(FailureCase {
    name: "missing_rule",
    manifest: include_str!("../data/missing_rule.yml"),
    expected: &[
        "Failed to build graph from the manifest.",
        "Rule 'missing' referenced by target 'hello.o' was not found.",
    ],
})]
#[case::duplicate_outputs(FailureCase {
    name: "duplicate_outputs",
    manifest: include_str!("../data/duplicate_outputs.yml"),
    expected: &[
        "Failed to build graph from the manifest.",
        "Duplicate outputs detected",
        "hello.o",
    ],
})]
#[case::circular_dependency(FailureCase {
    name: "circular_dependency",
    manifest: CIRCULAR_DEPENDENCY,
    expected: &["Circular dependency detected", "a.txt", "b.txt"],
})]
fn portable_failure_snapshot(#[case] case: FailureCase<'_>) -> Result<()> {
    assert_failure_snapshot(&case)
}

/// A carriage return in a script is refused only where Ninja text is literal.
///
/// Windows renders `script:` recipes through the encoded PowerShell transport,
/// whose Base64 payload carries a carriage return intact, so the rejection
/// belongs to the POSIX shell path. Gating the case keeps the portable table
/// from asserting different behaviour on each platform.
#[cfg(not(windows))]
#[test]
fn carriage_return_script_snapshot() -> Result<()> {
    assert_failure_snapshot(&FailureCase {
        name: "carriage_return_script",
        manifest: CARRIAGE_RETURN_SCRIPT,
        expected: &[
            "Failed to generate the Ninja manifest.",
            "unsafe Ninja control character",
        ],
    })
}

/// The README's rejected backtick command renders through the default output.
///
/// The rejection is decided only for POSIX and Bash command recipes, so the
/// Windows build has no equivalent failure to snapshot.
#[cfg(unix)]
#[test]
fn readme_backtick_interpolation_snapshot() -> Result<()> {
    assert_failure_snapshot(&FailureCase {
        name: "readme_backtick_interpolation",
        manifest: BACKTICK_INTERPOLATION,
        expected: &[
            "Failed to build graph from the manifest.",
            "Invalid command interpolation: recipe marker inside a protected shell region",
        ],
    })
}

/// A parenthesised case pattern cannot carry a marker out of `$()`.
///
/// This is the form that previously let the tracker close the substitution at
/// the pattern's `esac`, encode the arm's marker for the enclosing double
/// quotes, and place executable substitution text in the command. The CLI path
/// is the one users run, and the rejection is decided during graph
/// construction, so it needs no shell on the host. Only the POSIX route parses
/// `case`, so the Windows build has no equivalent failure to assert.
#[cfg(unix)]
#[test]
fn case_pattern_opener_interpolation_is_rejected() -> Result<()> {
    let manifest = concat!(
        "netsuke_version: '1.0.0'\n",
        "targets:\n",
        "  - name: out.txt\n",
        "    sources: 'x\"; touch injected; echo \"y'\n",
        "    command: \"echo \\\"$(case esac in (esac) printf %s {{ ins }} ;; esac)\\\"\"\n",
        "defaults: [out.txt]\n",
    );
    let workspace = workspace_with(manifest)?;
    let rendered = failing_generate(workspace.path())?;
    for text in [
        "Failed to build graph from the manifest.",
        "Invalid command interpolation: recipe marker inside a protected shell region",
    ] {
        ensure!(
            rendered.contains(text),
            "the opener rejection must report {text:?}, got: {rendered}"
        );
    }
    ensure!(
        !rendered.contains("touch injected"),
        "the diagnostic must not echo the hostile path, got: {rendered}"
    );
    Ok(())
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
