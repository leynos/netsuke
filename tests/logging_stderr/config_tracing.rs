//! Binary-level coverage for configuration-resolution tracing.
//!
//! The unit tests drive the tracing helpers in-process. These assert that the
//! events actually reach `stderr` of the real binary, which only happens if the
//! subscriber is installed before configuration is resolved, and that the
//! bounded field policy survives to the user-visible output.

use anyhow::{Context, Result, ensure};
use tempfile::{TempDir, tempdir};
use test_support::config_metrics::{MetricSnapshotRecord, assert_config_metrics_snapshot};
use test_support::netsuke::{run_netsuke_in, run_netsuke_in_with_env};

/// Lines emitted by the tracing subscriber, excluding the final error report.
///
/// Human-mode configuration failures and tracing events share stderr. The
/// terminal failure record carries only bounded operational fields; the privacy
/// assertions below therefore inspect both it and the deferred diagnostics.
///
/// Every tracing line begins with an ISO timestamp, whereas the error report's
/// continuation lines do not. Matching the full `YYYY-` prefix rather than a
/// leading digit keeps the report's numbered source snippet (`1 | ...`) out.
fn is_tracing_line(line: &str) -> bool {
    line.split_once('-').is_some_and(|(year, _)| {
        year.len() == 4 && year.chars().all(|digit| digit.is_ascii_digit())
    })
}

fn diagnostic_lines(stderr: &str) -> Vec<&str> {
    stderr
        .lines()
        .filter(|line| is_tracing_line(line))
        .filter(|line| !line.contains("configuration load failed"))
        .collect()
}

fn assert_config_load_failure(stderr: &str, operation: &str, error_category: &str) -> Result<()> {
    ensure!(
        stderr.contains("configuration load failed")
            && stderr.contains(&format!("operation=\"{operation}\""))
            && stderr.contains(&format!("error_category=\"{error_category}\"")),
        "stderr should identify the config-load operation and category: {stderr}"
    );
    Ok(())
}

const DIAG_MODE_FAILURE_METRICS: &[MetricSnapshotRecord] = &[
    MetricSnapshotRecord {
        name: "config_load_total",
        labels: &[
            "Label(\"phase\", \"diag_mode\")",
            "Label(\"outcome\", \"failure\")",
        ],
        value: Some("Counter(1)"),
    },
    MetricSnapshotRecord {
        name: "config_load_duration_seconds",
        labels: &["Label(\"phase\", \"diag_mode\")"],
        value: None,
    },
];

const MERGE_FAILURE_METRICS: &[MetricSnapshotRecord] = &[
    MetricSnapshotRecord {
        name: "config_load_total",
        labels: &[
            "Label(\"phase\", \"diag_mode\")",
            "Label(\"outcome\", \"success\")",
        ],
        value: Some("Counter(1)"),
    },
    MetricSnapshotRecord {
        name: "config_load_total",
        labels: &[
            "Label(\"phase\", \"merge\")",
            "Label(\"outcome\", \"failure\")",
        ],
        value: Some("Counter(1)"),
    },
    MetricSnapshotRecord {
        name: "config_load_duration_seconds",
        labels: &["Label(\"phase\", \"diag_mode\")"],
        value: None,
    },
    MetricSnapshotRecord {
        name: "config_load_duration_seconds",
        labels: &["Label(\"phase\", \"merge\")"],
        value: None,
    },
];

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
    let diagnostics = diagnostic_lines(&run.stderr);
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
    assert_config_metrics_snapshot(&run.stderr)?;
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
    let diagnostics = diagnostic_lines(&run.stderr);
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
    ensure!(
        !run.stderr.contains("missing-secret-name.toml")
            && !run.stderr.contains(raw_path.as_str())
            && !run.stderr.contains("explicit configuration file not found"),
        "human stderr must not expose configuration details: {}",
        run.stderr
    );
    assert_config_load_failure(&run.stderr, "diag_mode_resolution", "io")?;
    assert_config_metrics_snapshot(&run.stderr, DIAG_MODE_FAILURE_METRICS)?;
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

    ensure!(
        !run.success,
        "an invalid configuration environment value should fail the run"
    );
    assert_config_load_failure(&run.stderr, "config_merge", "validation")?;
    assert_config_metrics_snapshot(&run.stderr, MERGE_FAILURE_METRICS)?;
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
    let joined = diagnostic_lines(&run.stderr).join("\n");

    ensure!(
        !run.success,
        "an invalid explicit config file should fail the run"
    );

    ensure!(
        joined.contains("explicit config load failed") && joined.contains("failure_kind=LoadError"),
        "stderr should classify the parse failure: {joined}"
    );
    ensure!(
        !joined.contains("invalid parser secret"),
        "diagnostics must not echo the parser input: {joined}"
    );
    ensure!(
        !joined.contains(raw_path.as_str()),
        "diagnostics must not log the raw config path: {joined}"
    );
    ensure!(
        !run.stderr.contains("invalid-secret-config.toml")
            && !run.stderr.contains(raw_path.as_str())
            && !run.stderr.contains("invalid parser secret"),
        "human stderr must not expose configuration details: {}",
        run.stderr
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
