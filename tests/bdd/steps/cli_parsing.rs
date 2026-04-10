//! CLI parsing step definitions for BDD scenarios.
//!
//! This module provides steps for isolated CLI parsing tests that focus on
//! argument parsing behavior without interference from ambient configuration
//! files or environment variables.

use anyhow::{Context, Result};
use rstest_bdd_macros::{given, when};
use tempfile::tempdir;

use crate::bdd::fixtures::TestWorld;
use crate::bdd::helpers::env_mutation::mutate_env_var;
use crate::bdd::types::{CliArgs, EnvVarKey};

use super::cli::apply_cli;

/// Set up an isolated environment for generic CLI parsing tests.
///
/// This step ensures that CLI parsing tests are not affected by ambient host
/// configuration files or environment variables by:
/// 1. Creating a temporary directory and anchoring config discovery to it.
/// 2. Sandboxing user-scope config paths (`HOME`, `XDG_CONFIG_HOME`, `APPDATA`).
/// 3. Clearing all `NETSUKE_*` environment variables that could interfere.
///
/// Use this step for tests in `cli.feature` and `cli_config.feature` that
/// focus on parsing behaviour rather than configuration discovery.
#[given("an isolated CLI environment")]
fn isolated_cli_environment(world: &TestWorld) -> Result<()> {
    // Create a temporary directory to anchor config discovery
    let temp = tempdir().context("create temporary directory for CLI isolation")?;
    let temp_path_str = temp
        .path()
        .to_str()
        .context("temp directory path contains invalid UTF-8")?;

    // Sandbox user-scope config discovery paths to the temp directory
    mutate_env_var(world, EnvVarKey::from("HOME"), Some(temp_path_str))?;
    mutate_env_var(
        world,
        EnvVarKey::from("XDG_CONFIG_HOME"),
        Some(temp_path_str),
    )?;
    mutate_env_var(world, EnvVarKey::from("APPDATA"), Some(temp_path_str))?;

    *world.temp_dir.borrow_mut() = Some(temp);

    // Clear all NETSUKE_* environment variables to prevent interference
    let netsuke_vars = [
        "NETSUKE_CONFIG_PATH",
        "NETSUKE_THEME",
        "NETSUKE_LOCALE",
        "NETSUKE_JOBS",
        "NETSUKE_COLOUR_POLICY",
        "NETSUKE_SPINNER_MODE",
        "NETSUKE_OUTPUT_FORMAT",
        "NETSUKE_DEFAULT_TARGETS",
        "NETSUKE_FETCH_ALLOW_SCHEME",
        "NETSUKE_FETCH_ALLOW_HOST",
        "NETSUKE_FETCH_BLOCK_SCHEME",
        "NETSUKE_FETCH_BLOCK_HOST",
        "NETSUKE_DIAG_JSON",
        "NETSUKE_PROGRESS",
        "NETSUKE_ACCESSIBLE",
        "NETSUKE_NO_EMOJI",
    ];

    for var in &netsuke_vars {
        mutate_env_var(world, EnvVarKey::from(*var), None)?;
    }

    Ok(())
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macro generates Result wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[given("the CLI is parsed with {args:string}")]
#[when("the CLI is parsed with {args:string}")]
#[when("the CLI is parsed with invalid arguments {args:string}")]
fn parse_cli(world: &TestWorld, args: CliArgs) -> Result<()> {
    apply_cli(world, &args);
    Ok(())
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macro generates Result wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[when("the CLI is parsed with no additional arguments")]
fn parse_cli_no_args(world: &TestWorld) -> Result<()> {
    apply_cli(world, &CliArgs::from(""));
    Ok(())
}
