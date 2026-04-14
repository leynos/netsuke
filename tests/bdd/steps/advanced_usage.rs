//! Step definitions for advanced usage workflow scenarios.

use crate::bdd::fixtures::TestWorld;
use anyhow::{Context, Result};
use rstest_bdd_macros::given;
use std::fs;

/// Creates a `.netsuke.toml` configuration file in the workspace with the
/// specified key-value pair.
#[given("a workspace with config file setting {key} to {value}")]
fn given_config_file_with_setting(world: &TestWorld, key: String, value: String) -> Result<()> {
    let temp = world.temp_dir.borrow();
    let dir = temp.as_ref().context("temp dir has not been initialised")?;
    let config_path = dir.path().join(".netsuke.toml");

    // Serialize the value as TOML to handle quotes, backslashes, and newlines correctly
    let mut table = toml::map::Map::new();
    let toml_value = if value == "true" {
        toml::Value::Boolean(true)
    } else if value == "false" {
        toml::Value::Boolean(false)
    } else {
        toml::Value::String(value)
    };
    table.insert(key, toml_value);
    let toml_content = toml::to_string(&table).context("serialize TOML config")?;
    fs::write(&config_path, toml_content)
        .with_context(|| format!("write config file {}", config_path.display()))?;
    Ok(())
}
