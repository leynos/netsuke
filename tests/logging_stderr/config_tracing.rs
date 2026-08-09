//! Binary-level coverage for configuration-resolution tracing.
//!
//! The unit tests drive the tracing helpers in-process. These assert that the
//! events actually reach `stderr` of the real binary, which only happens if the
//! subscriber is installed before configuration is resolved, and that the
//! bounded field policy survives to the user-visible output.

use anyhow::{Context, Result, ensure};
use tempfile::{TempDir, tempdir};
use test_support::netsuke::{run_netsuke_in, run_netsuke_in_with_env};

#[path = "config_tracing_metrics.rs"]
mod config_tracing_metrics;
use config_tracing_metrics::{
    CONFIGURATION_MERGE_FAILURE_METRICS, ConfigurationDiagnosticOutput,
    DIAGNOSTIC_MODE_FAILURE_METRICS, SUCCESSFUL_EXPLICIT_SELECTION_METRICS,
    assert_config_metrics_snapshot,
};

struct ConfigLoadFailureExpectation {
    operation: &'static str,
    error_category: &'static str,
}

/// Lines emitted by the tracing subscriber, excluding the final error report.
///
/// Human-mode configuration failures and tracing events share stderr. The
/// terminal failure record carries only bounded operational fields; the privacy
/// assertions below therefore inspect both it and the deferred diagnostics.
///
/// Every tracing line begins with an ISO timestamp, whereas the error report's
/// continuation lines do not. Matching the full `YYYY-` prefix rather than a
/// leading digit keeps the report's numbered source snippet (`1 | ...`) out.
fn diagnostic_lines<'a>(output: &ConfigurationDiagnosticOutput<'a>) -> Vec<&'a str> {
    output
        .stderr
        .lines()
        .filter(|line| {
            line.split_once('-').is_some_and(|(year, _)| {
                year.len() == 4 && year.chars().all(|digit| digit.is_ascii_digit())
            })
        })
        .filter(|line| !line.contains("configuration load failed"))
        .collect()
}

fn assert_config_load_failure(
    output: &ConfigurationDiagnosticOutput<'_>,
    expected: &ConfigLoadFailureExpectation,
) -> Result<()> {
    ensure!(
        output.stderr.contains("configuration load failed")
            && output
                .stderr
                .contains(&format!("operation=\"{}\"", expected.operation))
            && output
                .stderr
                .contains(&format!("error_category=\"{}\"", expected.error_category)),
        "stderr should identify the config-load operation and category: {}",
        output.stderr
    );
    Ok(())
}

fn workspace() -> Result<TempDir> {
    let temp = tempdir().context("create temp dir")?;
    test_support::fs::write(
        temp.path().join("Netsukefile"),
        "netsuke_version: \"1.0.0\"\ntargets:\n  - name: noop\n    command: \"true\"\n",
    )
    .context("write manifest")?;
    Ok(temp)
}

/// A successful explicit selection is traced with bounded fields only.
#[test]
fn explicit_selection_traces_bounded_fields() -> Result<()> {
    let temp = workspace()?;
    let config = temp.path().join("customer@example.com.toml");
    test_support::fs::write(&config, "emoji = \"always\"\n").context("write config")?;
    let raw_path = config.to_string_lossy().into_owned();

    let run = run_netsuke_in(
        temp.path(),
        &["--verbose", "--config", raw_path.as_str(), "generate"],
    )?;
    ensure!(
        run.success,
        "an explicit configuration path should allow generate to succeed"
    );

    let output = ConfigurationDiagnosticOutput {
        stderr: &run.stderr,
    };
    let diagnostics = diagnostic_lines(&output);
    let joined = diagnostics.join("\n");

    ensure!(
        joined.contains("resolved config path") && joined.contains("selector=\"cli_flag\""),
        "stderr should name the winning selector: {joined}"
    );
    ensure!(
        joined.contains("using explicit config path"),
        "verbose startup should replay the cached explicit selection: {joined}"
    );
    ensure!(
        joined.contains("path_hash="),
        "stderr should carry the bounded path hash: {joined}"
    );
    ensure!(
        !joined.contains("customer@example.com.toml"),
        "verbose diagnostics must not expose the configuration file name: {joined}"
    );
    ensure!(
        !joined.contains(raw_path.as_str()),
        "diagnostics must not log the raw config path: {joined}"
    );
    ensure!(
        run.success,
        "a selected valid config should complete successfully"
    );
    assert_config_metrics_snapshot(&output, &SUCCESSFUL_EXPLICIT_SELECTION_METRICS)?;
    Ok(())
}

/// An explicit-load failure is traced with its failure kind, not the error text.
#[test]
fn explicit_load_failure_traces_failure_kind() -> Result<()> {
    let temp = workspace()?;
    let missing = temp.path().join("missing-secret-name.toml");
    let raw_path = missing.to_string_lossy().into_owned();

    let run = run_netsuke_in(
        temp.path(),
        &["--verbose", "--config", raw_path.as_str(), "generate"],
    )?;
    let output = ConfigurationDiagnosticOutput {
        stderr: &run.stderr,
    };
    let diagnostics = diagnostic_lines(&output);
    let joined = diagnostics.join("\n");

    ensure!(
        !run.success,
        "a missing explicit config file should fail the run"
    );
    ensure!(
        joined.contains("explicit config load failed") && joined.contains("failure_kind=Missing"),
        "stderr should classify the failure: {joined}"
    );
    ensure!(
        joined.contains("path_hash="),
        "stderr should carry the bounded path hash: {joined}"
    );
    ensure!(
        !joined.contains("missing-secret-name.toml"),
        "diagnostics must not expose the configuration file name: {joined}"
    );
    ensure!(
        !joined.contains(raw_path.as_str()),
        "diagnostics must not log the raw config path: {joined}"
    );
    ensure!(
        !joined.contains("explicit configuration file not found"),
        "diagnostics must not repeat the formatted error text: {joined}"
    );

    assert_config_load_failure(
        &output,
        &ConfigLoadFailureExpectation {
            operation: "diag_mode_resolution",
            error_category: "io",
        },
    )?;
    assert_config_metrics_snapshot(&output, &DIAGNOSTIC_MODE_FAILURE_METRICS)?;
    Ok(())
}

/// An environment validation failure reaches the full configuration merge.
#[test]
fn environment_validation_failure_identifies_config_merge() -> Result<()> {
    let temp = workspace()?;
    let run = run_netsuke_in_with_env(
        temp.path(),
        &["--verbose", "generate"],
        &[("NETSUKE_JOBS", "0")],
    )?;
    let output = ConfigurationDiagnosticOutput {
        stderr: &run.stderr,
    };

    ensure!(
        !run.success,
        "an invalid configuration environment value should fail the run"
    );
    assert_config_load_failure(
        &output,
        &ConfigLoadFailureExpectation {
            operation: "config_merge",
            error_category: "validation",
        },
    )?;
    assert_config_metrics_snapshot(&output, &CONFIGURATION_MERGE_FAILURE_METRICS)?;
    Ok(())
}

/// An invalid explicit file is traced without echoing the parser's input.
#[test]
fn invalid_config_traces_without_parser_text() -> Result<()> {
    let temp = workspace()?;
    let config = temp.path().join("invalid-secret-config.toml");
    test_support::fs::write(&config, "theme = [invalid parser secret\n").context("write config")?;
    let raw_path = config.to_string_lossy().into_owned();

    let run = run_netsuke_in(
        temp.path(),
        &["--verbose", "--config", raw_path.as_str(), "generate"],
    )?;
    let output = ConfigurationDiagnosticOutput {
        stderr: &run.stderr,
    };
    let joined = diagnostic_lines(&output).join("\n");

    ensure!(
        !run.success,
        "an invalid explicit config file should fail the run"
    );

    ensure!(
        joined.contains("explicit config load failed") && joined.contains("failure_kind=LoadError"),
        "stderr should classify the parse failure: {joined}"
    );
    ensure!(
        joined.contains("resolved config path") && joined.contains("selector=\"cli_flag\""),
        "verbose stderr should replay the cached selector decision: {joined}"
    );
    ensure!(
        !joined.contains("invalid parser secret"),
        "diagnostics must not echo the parser input: {joined}"
    );
    ensure!(
        !joined.contains(raw_path.as_str()),
        "diagnostics must not log the raw config path: {joined}"
    );
    Ok(())
}

/// JSON mode emits the diagnostic document and nothing else on stderr.
#[test]
fn json_mode_emits_no_tracing() -> Result<()> {
    let temp = workspace()?;
    let config = temp.path().join("selected.toml");
    test_support::fs::write(&config, "emoji = \"always\"\n").context("write config")?;

    let cli_json_run = run_netsuke_in(
        temp.path(),
        &[
            "--json",
            "--verbose",
            "--config",
            config.to_string_lossy().as_ref(),
            "generate",
        ],
    )?;
    test_support::fs::write(&config, "json = true\nemoji = \"always\"\n")
        .context("write JSON config")?;
    let config_json_run = run_netsuke_in(
        temp.path(),
        &[
            "--verbose",
            "--config",
            config.to_string_lossy().as_ref(),
            "generate",
        ],
    )?;

    for (source, run) in [("CLI", cli_json_run), ("config", config_json_run)] {
        ensure!(
            run.stderr.is_empty(),
            "{source}-selected JSON mode must leave stderr empty: {}",
            run.stderr
        );
        for marker in [
            "resolved config path",
            "read config path variable",
            "using explicit config path",
            "using config discovery",
        ] {
            ensure!(
                !run.stderr.contains(marker),
                "{source}-selected JSON mode must not emit tracing ({marker}): {}",
                run.stderr
            );
        }
    }
    Ok(())
}
