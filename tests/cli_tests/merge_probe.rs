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
use tempfile::TempDir;
use test_support::fs as test_fs;

const ARGS_ENV: &str = "NETSUKE_TEST_MERGE_PROBE_ARGS";
const OUTPUT_ENV: &str = "NETSUKE_TEST_MERGE_PROBE_OUTPUT";
const WORKER_NAME: &str = "cli_tests::merge_probe::merge_probe_worker";

#[derive(Deserialize, Serialize)]
struct ProbeResult {
    cli: Cli,
    /// Preserve the subcommand separately because `Cli::command` is skipped by
    /// Serde and therefore cannot survive the normal worker-result round trip.
    command: Option<netsuke::cli::Commands>,
}

/// Build the baseline environment for an isolated configuration probe.
///
/// # Errors
///
/// Returns an error if the empty system-configuration directory cannot be
/// created.
///
/// # Examples
///
/// ```rust,ignore
/// let project = tempfile::tempdir()?;
/// let overrides = [(
///     std::ffi::OsString::from("NETSUKE_EMOJI"),
///     std::ffi::OsString::from("always"),
/// )];
/// let (xdg_config_dirs, environment) = isolated_environment(project.path(), &overrides)?;
/// let merged = merge_in_child(&["netsuke"], project.path(), &environment)?;
/// assert_eq!(merged.emoji, netsuke::cli::EmojiPolicy::Always);
///
/// // Keep `xdg_config_dirs` alive until the child has finished. The
/// // XDG_CONFIG_DIRS value points into this directory; dropping it first
/// // removes the directory that the child uses for configuration discovery.
/// drop(xdg_config_dirs);
/// # Ok::<(), anyhow::Error>(())
/// ```
pub(super) fn isolated_environment(
    home: &Path,
    overrides: &[(OsString, OsString)],
) -> Result<(TempDir, Vec<(OsString, OsString)>)> {
    let xdg_config_dirs = tempfile::tempdir().context("create empty XDG_CONFIG_DIRS")?;
    let mut environment = vec![
        (OsString::from("HOME"), home.as_os_str().to_owned()),
        (
            OsString::from("XDG_CONFIG_HOME"),
            home.join(".config").into_os_string(),
        ),
        (
            OsString::from("XDG_CONFIG_DIRS"),
            xdg_config_dirs.path().as_os_str().to_owned(),
        ),
    ];
    environment.extend_from_slice(overrides);
    Ok((xdg_config_dirs, environment))
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
        .env_clear();
    let process_env = DefaultEnv;
    // `LD_LIBRARY_PATH` and `DYLD_FALLBACK_LIBRARY_PATH` are supplied by Cargo so
    // the test binary can find its dynamic dependencies; clearing them would
    // leave the re-executed worker unable to start on Linux and macOS.
    for key in [
        "SystemRoot",
        "PATH",
        "TEMP",
        "TMP",
        "LD_LIBRARY_PATH",
        "DYLD_FALLBACK_LIBRARY_PATH",
    ] {
        if let Some(value) = process_env.os_string(key) {
            command.env(key, value);
        }
    }
    command
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
    let mut merged = netsuke::cli::merge_with_config(&cli, &matches)
        .context("merge configuration in probe")?
        .with_default_command();
    // `Cli.command` is `#[serde(skip)]`, so it travels beside the CLI in
    // `ProbeResult::command`; taking it avoids cloning the subcommand tree.
    let command = merged.command.take();
    let encoded = serde_json::to_vec(&ProbeResult {
        cli: merged,
        command,
    })
    .context("encode merged CLI")?;
    test_fs::write(PathBuf::from(output_path), encoded).context("write merged CLI probe output")
}
