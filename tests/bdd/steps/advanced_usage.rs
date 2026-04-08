//! Step definitions for advanced usage workflow scenarios.

use crate::bdd::fixtures::TestWorld;
use crate::bdd::helpers::assertions::normalize_fluent_isolates;
use crate::bdd::types::OutputFragment;
use anyhow::{Context, Result, ensure};
use rstest_bdd_macros::{given, then};
use std::ffi::OsString;
use std::fs;

/// Creates a `.netsuke.toml` configuration file in the workspace with the
/// specified key-value pair.
#[given("a workspace with config file setting {key} to {value}")]
fn given_config_file_with_setting(world: &TestWorld, key: String, value: String) -> Result<()> {
    let temp = world.temp_dir.borrow();
    let dir = temp.as_ref().context("temp dir has not been initialised")?;
    let config_path = dir.path().join(".netsuke.toml");

    // Determine if the value is boolean, otherwise quote it as a string
    let toml_value = if value == "true" || value == "false" {
        value
    } else {
        format!("\"{value}\"")
    };

    let toml_content = format!("{key} = {toml_value}\n");
    fs::write(&config_path, toml_content)
        .with_context(|| format!("write config file {}", config_path.display()))?;
    Ok(())
}

/// Sets an environment variable for the netsuke invocation.
///
/// The environment variable is stored in the test world's `env_vars` map and
/// will be applied when the netsuke command is run.
#[given("the environment variable {name} is set to {value}")]
fn given_environment_variable(world: &TestWorld, name: String, value: String) {
    let mut env_vars = world.env_vars.borrow_mut();
    env_vars.insert(name, Some(OsString::from(value)));
}

/// Checks that stderr does not contain the specified fragment.
#[then("stderr should not contain {fragment}")]
fn then_stderr_not_contains(world: &TestWorld, fragment: OutputFragment) -> Result<()> {
    let stderr = world
        .command_stderr
        .get()
        .context("stderr should be captured")?;
    let normalized = normalize_fluent_isolates(&stderr);
    let normalized_fragment = normalize_fluent_isolates(fragment.as_str());

    ensure!(
        !normalized.contains(&normalized_fragment),
        "expected stderr to omit '{}', but it was present in:\n{}",
        fragment.as_str(),
        stderr
    );
    Ok(())
}
