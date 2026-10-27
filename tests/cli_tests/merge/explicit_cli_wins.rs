//! Preservation contract for explicit CLI values that equal the default.
//!
//! This lives beneath `merge` so it inherits that module's sanctioned ambient
//! filesystem exclusion: the test stages a temporary configuration file, which
//! is the same boundary `merge` itself occupies.

use anyhow::{Context, Result, ensure};
use netsuke::cli::CliConfig;
use rstest::rstest;
use std::ffi::OsString;
use std::fs;

use super::super::merge_probe::{isolated_environment, merge_in_child};

/// An explicit CLI value wins even when it equals the compiled default.
///
/// The v0.10.0 release corrects a defect where a generated CLI guard compared
/// parsed values against defaults and therefore discarded an explicit
/// `--flag <default>`. Netsuke does not use that guard — `push_cli_layer`
/// inserts a field when `ArgMatches::value_source` reports `CommandLine`, which
/// is per-field and value-independent — so this is a *preservation* contract,
/// not a behavioural change. It is pinned anyway: a later switch to the
/// generated loader would otherwise regress silently, because the merged value
/// would be indistinguishable from the default-application case.
///
/// `--manifest-fuel` is the vehicle because its clap `default_value_t` is
/// exactly `DEFAULT_MANIFEST_FUEL`. The file and environment layers both say
/// something different, so "CLI wins" is distinguishable from "default wins"
/// and from "env wins".
#[rstest]
fn explicit_cli_value_equal_to_the_default_still_wins() -> Result<()> {
    let temp_dir = tempfile::tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    fs::write(&config_path, "manifest_fuel = 7\n").context("write netsuke.toml")?;
    let (_xdg_config_dirs, environment) = isolated_environment(
        temp_dir.path(),
        &[
            (
                OsString::from("NETSUKE_CONFIG"),
                config_path.into_os_string(),
            ),
            (OsString::from("NETSUKE_MANIFEST_FUEL"), OsString::from("9")),
        ],
    )?;

    // Read the default from the compiled configuration rather than restating
    // it, so the case stays "explicit value == default" if the default moves.
    let default_fuel = CliConfig::default().manifest_fuel;
    let merged = merge_in_child(
        &["netsuke", "--manifest-fuel", &default_fuel.to_string()],
        temp_dir.path(),
        &environment,
    )?;

    ensure!(
        merged.manifest_fuel == default_fuel,
        "an explicit --manifest-fuel equal to the default ({default_fuel}) \
         should beat the file layer's 7 and the environment layer's 9, got {}",
        merged.manifest_fuel,
    );
    Ok(())
}
