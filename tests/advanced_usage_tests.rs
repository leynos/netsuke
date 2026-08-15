//! Integration tests for the advanced usage chapter workflows.
//!
//! These tests cover edge cases and unhappy paths not reached by the BDD
//! scenarios in `tests/features/advanced_usage.feature`. They validate
//! the documented behaviour of `clean`, `graph`, `generate`, configuration
//! layering, and JSON diagnostics.

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use serde_json::Value;
use std::path::Path;
use tempfile::{TempDir, tempdir};
use test_support::check_ninja::fake_ninja_check_build_file;
#[cfg(unix)]
use test_support::check_ninja::{ToolName, fake_ninja_expect_tool};
use test_support::fixture::setup_minimal_workspace;
use test_support::fs as test_fs;
use test_support::netsuke::run_netsuke_in_with_env;

/// Captured output from a netsuke invocation.
struct CommandOutput {
    stdout: String,
    stderr: String,
    success: bool,
}

/// Run `netsuke` in `current_dir` with supplied args and optional `NINJA_ENV`.
fn run_netsuke(
    current_dir: &Path,
    args: &[&str],
    ninja_env: Option<&Path>,
) -> Result<CommandOutput> {
    // Build environment variable list based on ninja_env parameter.
    // When ninja_env is provided, pass it via NINJA_ENV to the isolated runner.
    let ninja_env_owned = ninja_env.map(|p| p.to_string_lossy().into_owned());
    let extra_env: Vec<(&str, &str)> = ninja_env_owned
        .as_ref()
        .map(|s| vec![("NETSUKE_NINJA", s.as_str())])
        .unwrap_or_default();
    let run = run_netsuke_in_with_env(current_dir, args, &extra_env)?;
    Ok(CommandOutput {
        stdout: run.stdout,
        stderr: run.stderr,
        success: run.success,
    })
}

fn assert_json_success(output: &CommandOutput, expected_command: &str) -> Result<()> {
    ensure!(output.success, "{expected_command} should succeed");
    ensure!(
        output.stderr.is_empty(),
        "{expected_command} should keep stderr empty: {}",
        output.stderr
    );
    let document: Value =
        serde_json::from_str(&output.stdout).context("stdout should be valid JSON")?;
    ensure!(
        document.get("schema_version").and_then(Value::as_u64) == Some(1),
        "JSON result should use schema version 1: {document}"
    );
    ensure!(
        document.pointer("/result/command").and_then(Value::as_str) == Some(expected_command),
        "JSON result should identify the {expected_command} command: {document}"
    );
    ensure!(
        document
            .pointer("/result/content")
            .is_some_and(Value::is_null),
        "JSON result content should be null: {document}"
    );
    Ok(())
}

type NinjaSetup = fn() -> Result<(TempDir, std::path::PathBuf)>;

struct JsonSubcommandRequest {
    context: &'static str,
    command: &'static str,
    make_ninja: NinjaSetup,
}

impl JsonSubcommandRequest {
    const fn into_parts(self) -> (&'static str, &'static str, NinjaSetup) {
        (self.context, self.command, self.make_ninja)
    }
}

fn assert_json_subcommand_success(request: JsonSubcommandRequest) -> Result<()> {
    let (context, command, make_ninja) = request.into_parts();
    let workspace = setup_minimal_workspace(Path::new(env!("CARGO_MANIFEST_DIR")), context)?;
    let (_ninja_dir, ninja_path) = make_ninja()?;
    let output = run_netsuke(
        workspace.path(),
        &["--json", command],
        Some(ninja_path.as_path()),
    )?;
    assert_json_success(&output, command)
}

#[cfg(unix)]
fn make_clean_ninja() -> Result<(TempDir, std::path::PathBuf)> {
    fake_ninja_expect_tool(ToolName::new("clean"))
}

struct ConfigLayerBuildRequest<'a> {
    context: &'a str,
    config_content: &'a str,
    args: &'a [&'a str],
    extra_env: &'a [(&'a str, &'a str)],
}

impl<'a> ConfigLayerBuildRequest<'a> {
    const fn into_parts(self) -> (&'a str, &'a str, &'a [&'a str], &'a [(&'a str, &'a str)]) {
        (self.context, self.config_content, self.args, self.extra_env)
    }
}

/// Shared workspace setup for configuration-layering tests.
///
/// Creates a minimal workspace, writes `config_content` to `.netsuke.toml`,
/// installs a fake ninja binary, and runs netsuke with the given `args` and
/// `extra_env`.  Returns the captured [`CommandOutput`].
fn run_config_layer_build(request: ConfigLayerBuildRequest<'_>) -> Result<CommandOutput> {
    let (context, config_content, args, extra_env) = request.into_parts();
    let workspace = setup_minimal_workspace(Path::new(env!("CARGO_MANIFEST_DIR")), context)?;
    let config = workspace.path().join(".netsuke.toml");
    std::fs::write(&config, config_content).context("write config file")?;
    let (_ninja_dir, ninja_path) = fake_ninja_check_build_file()?;
    run_netsuke_with_env(
        workspace.path(),
        args,
        Some(ninja_path.as_path()),
        extra_env,
    )
}

struct MetricRecordExpectation {
    record: &'static str,
    value: &'static str,
}

struct HistogramRecordExpectation {
    record: &'static str,
}

impl MetricRecordExpectation {
    const fn into_parts(self) -> (&'static str, &'static str) {
        (self.record, self.value)
    }
}

impl HistogramRecordExpectation {
    const fn into_record(self) -> &'static str {
        self.record
    }
}

fn contains_metric_record(snapshot: &str, expected: MetricRecordExpectation) -> bool {
    let (expected_record, expected_value) = expected.into_parts();
    snapshot
        .split("), (")
        .any(|record| record.contains(expected_record) && record.contains(expected_value))
}

fn contains_non_empty_histogram_record(
    snapshot: &str,
    expected: HistogramRecordExpectation,
) -> bool {
    let expected_record = expected.into_record();
    snapshot.split("), (").any(|record| {
        record.contains(expected_record)
            && record.contains("Histogram([")
            && !record.contains("Histogram([])")
    })
}

fn assert_config_metrics_snapshot(stderr: &str) -> Result<()> {
    let snapshot = stderr
        .lines()
        .find(|line| line.contains("metrics snapshot"))
        .ok_or_else(|| anyhow::anyhow!("expected metrics snapshot in stderr: {stderr}"))?;
    let expected_counters = [
        MetricRecordExpectation {
            record: concat!(
                r#"CompositeKey(Counter, Key { name: KeyName("config_load_total"), labels: ["#,
                r#"Label("phase", "diag_mode"), Label("outcome", "success")]"#,
            ),
            value: "Counter(1)",
        },
        MetricRecordExpectation {
            record: concat!(
                r#"CompositeKey(Counter, Key { name: KeyName("config_load_total"), labels: ["#,
                r#"Label("phase", "merge"), Label("outcome", "success")]"#,
            ),
            value: "Counter(1)",
        },
    ];
    let expected_histograms = [
        HistogramRecordExpectation {
            record: concat!(
                r#"CompositeKey(Histogram, Key { name: KeyName("config_load_duration_seconds"), labels: ["#,
                r#"Label("phase", "diag_mode")]"#,
            ),
        },
        HistogramRecordExpectation {
            record: concat!(
                r#"CompositeKey(Histogram, Key { name: KeyName("config_load_duration_seconds"), labels: ["#,
                r#"Label("phase", "merge")]"#,
            ),
        },
    ];
    ensure!(
        snapshot.matches(r#"KeyName("config_load_total")"#).count() == expected_counters.len(),
        "expected exactly two configuration-load counter records: {snapshot}",
    );
    ensure!(
        snapshot
            .matches(r#"KeyName("config_load_duration_seconds")"#)
            .count()
            == expected_histograms.len(),
        "expected exactly two configuration-load duration records: {snapshot}",
    );
    for expected_counter in expected_counters {
        let expected_record = expected_counter.record;
        ensure!(
            contains_metric_record(snapshot, expected_counter),
            "expected counter {:?} in snapshot: {snapshot}",
            expected_record,
        );
    }
    for expected_histogram in expected_histograms {
        let expected_record = expected_histogram.record;
        ensure!(
            contains_non_empty_histogram_record(snapshot, expected_histogram),
            "expected histogram {:?} in snapshot: {snapshot}",
            expected_record,
        );
    }
    Ok(())
}

// -------------------------------------------------------------------------
// Clean subcommand edge cases
// -------------------------------------------------------------------------

#[cfg(unix)]
#[rstest]
fn clean_without_prior_build_handles_gracefully() -> Result<()> {
    let workspace = setup_minimal_workspace(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        "clean without prior build",
    )?;
    let (_ninja_dir, ninja_path) = fake_ninja_expect_tool(ToolName::new("clean"))?;

    let output = run_netsuke(workspace.path(), &["clean"], Some(ninja_path.as_path()))?;

    ensure!(
        output.success,
        "clean without a prior build should dispatch the clean tool successfully: {}",
        output.stderr
    );
    Ok(())
}

#[rstest]
fn build_json_emits_success_result() -> Result<()> {
    assert_json_subcommand_success(JsonSubcommandRequest {
        context: "JSON build success",
        command: "build",
        make_ninja: fake_ninja_check_build_file,
    })
}

#[cfg(unix)]
#[rstest]
fn clean_json_dispatches_tool_and_emits_success_result() -> Result<()> {
    assert_json_subcommand_success(JsonSubcommandRequest {
        context: "JSON clean success",
        command: "clean",
        make_ninja: make_clean_ninja,
    })
}

// -------------------------------------------------------------------------
// Graph subcommand edge cases
// -------------------------------------------------------------------------

#[test]
fn graph_with_invalid_manifest_fails_with_actionable_error() -> Result<()> {
    let workspace = tempdir().context("create temp dir for graph invalid manifest")?;
    let manifest = workspace.path().join("Netsukefile");
    std::fs::write(&manifest, "not: valid: yaml: [[[").context("write invalid manifest")?;

    let output = run_netsuke(workspace.path(), &["graph"], None)?;

    ensure!(
        !output.success,
        "expected graph with invalid manifest to fail"
    );
    ensure!(
        !output.stderr.is_empty(),
        "expected an error message on stderr"
    );
    Ok(())
}

// -------------------------------------------------------------------------
// Generate subcommand edge cases
// -------------------------------------------------------------------------

#[test]
fn generate_to_unwritable_path_fails_with_path_error() -> Result<()> {
    let workspace = setup_minimal_workspace(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        "generate to unwritable path",
    )?;
    // Create a regular file that blocks the parent directory creation.
    let blocker = workspace.path().join("blocker");
    test_fs::write(&blocker, "").context("create blocker file")?;
    let bad_path = blocker.join("out.ninja");

    let output = run_netsuke(
        workspace.path(),
        &[
            "generate",
            "--output",
            bad_path.to_str().expect("path is UTF-8"),
        ],
        None,
    )?;

    ensure!(
        !output.success,
        "expected generate to unwritable path to fail"
    );
    ensure!(
        output.stderr.contains("blocker"),
        "expected path-related error mentioning 'blocker' on stderr, got:\n{}",
        output.stderr
    );
    Ok(())
}

#[test]
fn generate_to_missing_parent_directory_succeeds_by_creating_parents() -> Result<()> {
    let workspace = setup_minimal_workspace(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        "generate to missing parent",
    )?;
    // Netsuke automatically creates missing parent directories.
    let nested_path = workspace.path().join("missing_parent").join("out.ninja");

    let output = run_netsuke(
        workspace.path(),
        &[
            "generate",
            "--output",
            nested_path.to_str().expect("path is UTF-8"),
        ],
        None,
    )?;

    ensure!(
        output.success,
        "expected generate to succeed and create parent directories"
    );
    ensure!(
        nested_path.exists(),
        "expected manifest file to be created at {}",
        nested_path.display()
    );
    Ok(())
}

// -------------------------------------------------------------------------

// Configuration layering precedence
// -------------------------------------------------------------------------

struct ConfigPrecedenceCase<'a> {
    config_content: &'a str,
    args: &'a [&'a str],
    extra_env: &'a [(&'a str, &'a str)],
    expect_verbose_diagnostics: bool,
}

#[rstest]
#[case::config_file_enables(ConfigPrecedenceCase {
    config_content: "verbose = true\n",
    args: &["build"],
    extra_env: &[],
    expect_verbose_diagnostics: true,
})]
#[case::env_var_enables(ConfigPrecedenceCase {
    config_content: "verbose = false\n",
    args: &["build"],
    extra_env: &[("NETSUKE_VERBOSE", "true")],
    expect_verbose_diagnostics: true,
})]
#[case::env_var_overrides_config(ConfigPrecedenceCase {
    config_content: "verbose = true\n",
    args: &["build"],
    extra_env: &[("NETSUKE_VERBOSE", "false")],
    expect_verbose_diagnostics: false,
})]
#[case::cli_flag_overrides_env(ConfigPrecedenceCase {
    config_content: "verbose = true\n",
    args: &["--verbose", "build"],
    extra_env: &[("NETSUKE_VERBOSE", "false")],
    expect_verbose_diagnostics: true,
})]
fn verbose_config_precedence(#[case] case: ConfigPrecedenceCase<'_>) -> Result<()> {
    let output = run_config_layer_build(ConfigLayerBuildRequest {
        context: "verbose config precedence",
        config_content: case.config_content,
        args: case.args,
        extra_env: case.extra_env,
    })?;
    ensure!(output.success, "expected build to succeed");
    if case.expect_verbose_diagnostics {
        ensure!(
            output.stderr.contains("Timing"),
            "expected verbose timing summary in stderr, got:\n{}",
            output.stderr
        );
        assert_config_metrics_snapshot(&output.stderr)?;
    } else {
        ensure!(
            !output.stderr.contains("Timing"),
            "expected no timing summary, got:\n{}",
            output.stderr
        );
        ensure!(
            !output.stderr.contains("metrics snapshot"),
            "expected no metrics snapshot, got:\n{}",
            output.stderr
        );
    }
    Ok(())
}

// -------------------------------------------------------------------------
// JSON diagnostics edge cases
// -------------------------------------------------------------------------

#[test]
fn json_diagnostics_with_verbose_produces_valid_json() -> Result<()> {
    let workspace = tempdir().context("create temp dir for JSON diagnostics verbose")?;
    let manifest = workspace.path().join("Netsukefile");
    std::fs::write(&manifest, "not_valid_yaml: [[[").context("write invalid manifest")?;

    let output = run_netsuke(workspace.path(), &["--json", "--verbose", "build"], None)?;

    ensure!(
        !output.success,
        "expected build with invalid manifest to fail"
    );
    // stderr should contain a valid JSON diagnostics envelope (possibly
    // multiline) without tracing noise leaking through.
    let trimmed = output.stderr.trim();
    ensure!(!trimmed.is_empty(), "expected JSON diagnostics on stderr");
    let parsed: serde_json::Value =
        serde_json::from_str(trimmed).context("expected stderr to be a valid JSON document")?;
    ensure!(
        parsed.get("diagnostics").is_some(),
        "expected a 'diagnostics' key in the JSON envelope"
    );
    // stdout should be empty when diagnostics go to stderr
    ensure!(
        output.stdout.trim().is_empty(),
        "expected stdout to be empty with --json, got:\n{}",
        output.stdout
    );
    Ok(())
}

#[test]
fn generate_to_stdout_contains_ninja_rules() -> Result<()> {
    let workspace =
        setup_minimal_workspace(Path::new(env!("CARGO_MANIFEST_DIR")), "generate to stdout")?;

    let output = run_netsuke(workspace.path(), &["generate"], None)?;

    ensure!(output.success, "expected generate to stdout to succeed");
    ensure!(
        output.stdout.contains("rule "),
        "expected stdout to contain Ninja rule statements, got:\n{}",
        output.stdout
    );
    // Progress output goes to stderr; the generated content goes to stdout.
    ensure!(
        !output.stderr.contains("rule "),
        "expected progress messages on stderr, not manifest content, got:\n{}",
        output.stderr
    );
    Ok(())
}
