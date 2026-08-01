//! Binary-level coverage for configuration-resolution tracing.
//!
//! The unit tests drive the tracing helpers in-process. These assert that the
//! events actually reach `stderr` of the real binary, which only happens if the
//! subscriber is installed before configuration is resolved, and that the
//! bounded field policy survives to the user-visible output.

use anyhow::{Context, Result, ensure};
use tempfile::{TempDir, tempdir};
use test_support::netsuke::run_netsuke_in;

/// Lines emitted by the tracing subscriber, excluding the final error report.
///
/// `config_err_to_exit` reports the failing configuration file by path and, for
/// a malformed file, renders the parser's own diagnostic. A user cannot act on
/// "configuration load failed" alone, so that report is the actionable error
/// rather than a bounded diagnostic; the privacy assertions here apply to the
/// diagnostic events, which is where the field policy is enforced.
///
/// Every tracing line begins with an ISO timestamp, whereas the error report's
/// continuation lines do not. Matching the full `YYYY-` prefix rather than a
/// leading digit keeps the report's numbered source snippet (`1 | ...`) out.
fn is_tracing_line(line: &str) -> bool {
    let bytes = line.as_bytes();
    bytes.len() > 4 && bytes[..4].iter().all(u8::is_ascii_digit) && bytes[4] == b'-'
}

fn diagnostic_lines(stderr: &str) -> Vec<&str> {
    stderr
        .lines()
        .filter(|line| is_tracing_line(line))
        .filter(|line| !line.contains("configuration load failed"))
        .collect()
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
    let config = temp.path().join("selected-secret-name.toml");
    test_support::fs::write(&config, "emoji = \"always\"\n").context("write config")?;
    let raw_path = config.to_string_lossy().into_owned();

    let run = run_netsuke_in(
        temp.path(),
        &["--verbose", "--config", raw_path.as_str(), "generate"],
    )?;
    let diagnostics = diagnostic_lines(&run.stderr);
    let joined = diagnostics.join("\n");

    ensure!(
        joined.contains("resolved config path") && joined.contains("selector=\"cli_flag\""),
        "stderr should name the winning selector: {joined}"
    );
    ensure!(
        joined.contains("path_hash=") && joined.contains("path_file_name="),
        "stderr should carry the bounded path fields: {joined}"
    );
    ensure!(
        joined.contains("selected-secret-name.toml"),
        "the bounded file name should be present: {joined}"
    );
    ensure!(
        !joined.contains(raw_path.as_str()),
        "diagnostics must not log the raw config path: {joined}"
    );
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
        joined.contains("path_hash=") && joined.contains("missing-secret-name.toml"),
        "stderr should carry the bounded path fields: {joined}"
    );
    ensure!(
        !joined.contains(raw_path.as_str()),
        "diagnostics must not log the raw config path: {joined}"
    );
    ensure!(
        !joined.contains("explicit configuration file not found"),
        "diagnostics must not repeat the formatted error text: {joined}"
    );
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
    Ok(())
}

/// JSON mode emits the diagnostic document and nothing else on stderr.
#[test]
fn json_mode_emits_no_tracing() -> Result<()> {
    let temp = workspace()?;
    let config = temp.path().join("selected.toml");
    test_support::fs::write(&config, "emoji = \"always\"\n").context("write config")?;

    let run = run_netsuke_in(
        temp.path(),
        &[
            "--json",
            "--verbose",
            "--config",
            config.to_string_lossy().as_ref(),
            "generate",
        ],
    )?;

    for marker in [
        "resolved config path",
        "read config path variable",
        "using explicit config path",
        "using config discovery",
    ] {
        ensure!(
            !run.stderr.contains(marker),
            "JSON mode must not emit tracing ({marker}): {}",
            run.stderr
        );
    }
    Ok(())
}
