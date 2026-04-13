//! Step definitions for advanced usage workflow scenarios.

use crate::bdd::fixtures::TestWorld;
use anyhow::{Context, Result};
use rstest_bdd_macros::given;
use std::ffi::{OsStr, OsString};
use std::fs;
use test_support::env::set_var;

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
/// The environment variable is set in the current process and the previous
/// value is tracked for restoration after the scenario completes.
#[given("the environment variable {name} is set to {value}")]
fn given_environment_variable(world: &TestWorld, name: String, value: String) {
    let new_val = OsString::from(&value);
    let previous = set_var(&name, OsStr::new(&value));
    world.track_env_var(name, previous, Some(new_val));
}
