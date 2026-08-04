//! Child-process adapter for configuration merge integration tests.
//!
//! The worker executes the real ambient configuration adapters in a dedicated
//! process. Parent tests configure that process with `Command::env`, keeping
//! the test harness itself free of process-environment mutation.

use anyhow::{Context, Result, ensure};
use mockable::{DefaultEnv, Env};
use netsuke::cli::Cli;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use test_support::fs as test_fs;

const ARGS_ENV: &str = "NETSUKE_TEST_MERGE_PROBE_ARGS";
const OUTPUT_ENV: &str = "NETSUKE_TEST_MERGE_PROBE_OUTPUT";
const WORKER_NAME: &str = "cli_tests::merge_probe::merge_probe_worker";

#[derive(Deserialize, Serialize)]
struct ProbeResult {
    cli: Cli,
    command: Option<netsuke::cli::Commands>,
}

/// Merge configuration in an isolated process with the supplied environment.
pub(super) fn merge_in_child(
    args: &[&str],
    current_dir: &Path,
    environment: &[(OsString, OsString)],
) -> Result<Cli> {
    let output_dir = tempfile::tempdir().context("create merge-probe output directory")?;
    let output_path = output_dir.path().join("merged.json");
    let encoded_args = serde_json::to_string(args).context("encode merge-probe arguments")?;
    let mut command = Command::new(std::env::current_exe().context("locate CLI test binary")?);
    command
        .args(["--ignored", "--exact", WORKER_NAME])
        .current_dir(current_dir)
        .env_clear()
        .env(ARGS_ENV, encoded_args)
        .env(OUTPUT_ENV, &output_path);
    for (key, value) in environment {
        command.env(key.as_os_str(), value);
    }
    let output = command.output().context("run configuration merge probe")?;
    ensure!(
        output.status.success(),
        "configuration merge probe failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let merged = test_fs::read(&output_path).context("read merged configuration probe output")?;
    let mut result: ProbeResult =
        serde_json::from_slice(&merged).context("decode merged configuration probe output")?;
    result.cli.command = result.command;
    Ok(result.cli)
}

#[test]
#[ignore = "invoked as a configuration merge worker"]
fn merge_probe_worker() -> Result<()> {
    let process_env = DefaultEnv;
    let encoded_args = process_env
        .raw(ARGS_ENV)
        .context("read merge-probe arguments")?;
    let output_path = process_env
        .os_string(OUTPUT_ENV)
        .context("read merge-probe output path")?;
    let args: Vec<String> =
        serde_json::from_str(&encoded_args).context("decode probe arguments")?;
    let localizer = Arc::from(netsuke::cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(args, &localizer)
        .context("parse CLI in merge probe")?;
    let merged = netsuke::cli::merge_with_config(&cli, &matches)
        .context("merge configuration in probe")?
        .with_default_command();
    let command = merged.command.clone();
    let encoded = serde_json::to_vec(&ProbeResult {
        cli: merged,
        command,
    })
    .context("encode merged CLI")?;
    test_fs::write(PathBuf::from(output_path), encoded).context("write merged CLI probe output")
}
