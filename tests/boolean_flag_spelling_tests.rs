//! Process-level contracts for the spelling of boolean command-line flags.
//!
//! `ortho_config` 0.10.0 taught its *generated* parsers to accept
//! `--flag=<BOOL>` with an optional value. Netsuke does not use a generated
//! parser: `src/cli/command.rs` declares every boolean as a plain
//! `#[arg(long)]` switch. These tests therefore record preservation, not new
//! behaviour. They exist because the migration changed the metadata that
//! *describes* those flags, and a future move to the generated loader would
//! otherwise be able to change what the binary accepts without any test
//! noticing.
//!
//! Exit status and stream routing are process-level facts, so every case runs
//! the real binary through `assert_cmd`. The child's environment is cleared and
//! pointed at a sandbox `HOME` so host configuration cannot change the parsed
//! values.

use anyhow::{Context, Result, ensure};
use assert_cmd::cargo::cargo_bin_cmd;
use rstest::rstest;
use std::path::Path;
use std::process::Output;
use tempfile::TempDir;
use test_support::fluent::normalize_fluent_isolates;
use test_support::fs as test_fs;

/// Create a workspace holding the minimal manifest and an empty sandboxed home.
fn sandbox() -> Result<(TempDir, TempDir)> {
    let workspace = tempfile::tempdir().context("create boolean-spelling workspace")?;
    let home = tempfile::tempdir().context("create empty boolean-spelling home")?;
    test_fs::copy(
        "tests/data/minimal.yml",
        workspace.path().join("Netsukefile"),
    )?;
    Ok((workspace, home))
}

/// Run `netsuke` in the sandbox with the supplied arguments.
fn run_netsuke(workspace: &Path, home: &Path, arguments: &[&str]) -> Result<Output> {
    let mut command = cargo_bin_cmd!("netsuke");
    command
        .current_dir(workspace)
        .env_clear()
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("XDG_CONFIG_DIRS", home.join("empty-system-config"))
        .args(arguments);
    command
        .output()
        .context("run netsuke with boolean arguments")
}

/// Assert a case's status code and that standard output stayed empty.
fn assert_rejected(output: &Output, expected_status: i32, case: &str) -> Result<String> {
    ensure!(
        output.status.code() == Some(expected_status),
        "{case} should exit {expected_status}, got {:?}",
        output.status.code(),
    );
    ensure!(
        output.stdout.is_empty(),
        "{case} should write nothing to stdout, got {} bytes",
        output.stdout.len(),
    );
    Ok(normalize_fluent_isolates(&String::from_utf8_lossy(
        &output.stderr,
    )))
}

/// Assert a case succeeded and return its standard output.
fn assert_succeeded(output: &Output, case: &str) -> Result<String> {
    ensure!(
        output.status.code() == Some(0),
        "{case} should exit 0, got {:?}; stderr: {}",
        output.status.code(),
        normalize_fluent_isolates(&String::from_utf8_lossy(&output.stderr)),
    );
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// A bare root-level switch is accepted and reaches the merged configuration.
///
/// `--verbose` is the vehicle because it has an observable effect on its own:
/// verbose mode emits `TRACE`-level events that a non-verbose run suppresses.
/// Rendering the empty manifest therefore proves the flag was parsed *and*
/// applied, rather than merely tolerated by clap.
#[rstest]
fn a_bare_root_switch_is_accepted_and_applied() -> Result<()> {
    let (workspace, home) = sandbox()?;
    let output = run_netsuke(workspace.path(), home.path(), &["--verbose", "graph"])?;
    assert_succeeded(&output, "a bare --verbose switch")?;

    let stderr = normalize_fluent_isolates(&String::from_utf8_lossy(&output.stderr));
    ensure!(
        stderr.contains("TRACE"),
        "--verbose should raise the event filter to TRACE, got: {stderr}"
    );
    Ok(())
}

/// A bare switch on a subcommand is accepted and changes the artefact.
///
/// `graph --html` switches the rendered document from DOT to a self-contained
/// HTML page, so the produced bytes prove the flag was read rather than
/// defaulted.
#[rstest]
fn a_bare_subcommand_switch_is_accepted_and_applied() -> Result<()> {
    let (workspace, home) = sandbox()?;
    let output = run_netsuke(
        workspace.path(),
        home.path(),
        &["graph", "--html", "--output", "-"],
    )?;
    let stdout = assert_succeeded(&output, "a bare --html switch")?;
    ensure!(
        stdout.starts_with("<!doctype html>"),
        "--html should render an HTML document, got: {stdout:.60}"
    );
    Ok(())
}

/// The `--flag=false` spelling is rejected as an unexpected value.
///
/// Clap reports this as an invalid-value error, so the process exits 2 and
/// prints `unexpected value 'false'` on standard error. The message names the
/// flag, which distinguishes this case from a generally broken invocation.
#[rstest]
#[case::verbose(&["--verbose=false", "graph"])]
#[case::json(&["--json=false", "graph"])]
#[case::fetch_default_deny(&["--fetch-default-deny=false", "graph"])]
#[case::trust_project_fetch_policy(&["--trust-project-fetch-policy=false", "graph"])]
#[case::no_input(&["--no-input=false", "graph"])]
#[case::html(&["graph", "--html=false"])]
fn an_equals_attached_value_is_rejected_as_an_unexpected_value(
    #[case] arguments: &[&str],
) -> Result<()> {
    let (workspace, home) = sandbox()?;
    let output = run_netsuke(workspace.path(), home.path(), arguments)?;
    let case = arguments.join(" ");
    let stderr = assert_rejected(&output, 2, &case)?;
    ensure!(
        stderr.contains("unexpected value 'false'"),
        "{case} should report an unexpected value, got: {stderr}"
    );
    Ok(())
}

/// The `--flag false` spelling is rejected too, but as a stray subcommand.
///
/// Because the switch takes no value, clap consumes only the flag and then
/// reads `false` as the next positional word. At the root that word is a
/// subcommand, and inside `graph` it is a stray argument, so the two tiers
/// report different errors. Both exit 2 and neither writes to standard output.
#[rstest]
#[case::root_switch(&["--verbose", "false", "graph"], "Unknown subcommand")]
#[case::subcommand_switch(&["graph", "--html", "false"], "Unknown argument")]
fn a_space_separated_value_is_rejected_as_a_stray_word(
    #[case] arguments: &[&str],
    #[case] expected_message: &str,
) -> Result<()> {
    let (workspace, home) = sandbox()?;
    let output = run_netsuke(workspace.path(), home.path(), arguments)?;
    let case = arguments.join(" ");
    let stderr = assert_rejected(&output, 2, &case)?;
    ensure!(
        stderr.contains(expected_message) && stderr.contains("false"),
        "{case} should report {expected_message:?} naming `false`, got: {stderr}"
    );
    Ok(())
}

/// `--json false` is a different case: the stray word is accepted first.
///
/// The pre-clap JSON hint scanner (`src/cli/l10n/mod.rs::json_hint_from_args`)
/// only recognises a bare `--json`, and it runs before clap. With `--json`
/// present the scanner selects JSON diagnostics, so the *subcommand* error is
/// rendered as a JSON document on standard error. This is the one space
/// separated case that exits 1 rather than 2, and the reason is routing, not
/// acceptance: the `false` word is still rejected.
#[rstest]
fn json_with_a_space_separated_value_keeps_its_json_diagnostics() -> Result<()> {
    let (workspace, home) = sandbox()?;
    let output = run_netsuke(workspace.path(), home.path(), &["--json", "false", "graph"])?;
    let stderr = assert_rejected(&output, 1, "--json false graph")?;

    let document: serde_json::Value =
        serde_json::from_str(&stderr).context("--json false should route a JSON document")?;
    let message = document
        .pointer("/diagnostics/0/message")
        .and_then(serde_json::Value::as_str)
        .context("JSON diagnostics should carry a message")?;
    ensure!(
        message.contains("Unknown subcommand") && message.contains("false"),
        "--json false should still reject `false`: {message}"
    );
    Ok(())
}
