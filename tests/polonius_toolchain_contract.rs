//! Contract tests pinning the Polonius toolchain plumbing.
//!
//! The tree only borrow-checks under `-Zpolonius=next` on the dated nightly
//! pinned in `rust-toolchain.toml` (see ADR-006 and docs/polonius.md). An
//! inherited `RUSTFLAGS` environment variable overrides the
//! `.cargo/config.toml` `build.rustflags` table, so every Makefile recipe
//! that sets `RUSTFLAGS` and every workflow that presets it must re-state
//! the flag. These tests fail when any layer drops the pin or the flag, so
//! a regression cannot reach CI as a confusing borrow-check error.

#[path = "support/makefile.rs"]
mod makefile;

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use makefile::{read_repo_file, target_recipe};
use rstest::rstest;
use serde_yaml::Value as YamlValue;
use toml::Value as TomlValue;

const POLONIUS_FLAG: &str = "-Zpolonius=next";
const POLONIUS_VAR: &str = "$(POLONIUS_FLAGS)";

/// Returns the dated nightly channel pinned in `rust-toolchain.toml`.
///
/// The workflow assertions compare against this value so a future pin move
/// only has to update `rust-toolchain.toml` and the workflows together.
fn pinned_toolchain() -> Result<String> {
    let manifest: TomlValue = read_repo_file(Utf8Path::new("rust-toolchain.toml"))?
        .parse()
        .context("parse rust-toolchain.toml")?;
    let channel = manifest
        .get("toolchain")
        .and_then(|toolchain| toolchain.get("channel"))
        .and_then(TomlValue::as_str)
        .context("rust-toolchain.toml should pin a toolchain channel")?;
    Ok(channel.to_owned())
}

/// Walks nested YAML mappings and returns the string at the key path.
fn yaml_str<'a>(value: &'a YamlValue, keys: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for key in keys {
        current = current.get(key)?;
    }
    current.as_str()
}

#[test]
fn rust_toolchain_pins_dated_nightly() -> Result<()> {
    let channel = pinned_toolchain()?;
    ensure!(
        channel.starts_with("nightly-20"),
        "rust-toolchain.toml should pin a dated nightly, found {channel:?}"
    );
    Ok(())
}

#[test]
fn cargo_config_enables_polonius_by_default() -> Result<()> {
    let config: TomlValue = read_repo_file(Utf8Path::new(".cargo/config.toml"))?
        .parse()
        .context("parse .cargo/config.toml")?;
    let rustflags = config
        .get("build")
        .and_then(|build| build.get("rustflags"))
        .and_then(TomlValue::as_array)
        .context(".cargo/config.toml should declare build.rustflags")?;
    ensure!(
        rustflags
            .iter()
            .any(|flag| flag.as_str() == Some(POLONIUS_FLAG)),
        "build.rustflags should enable {POLONIUS_FLAG}, found {rustflags:?}"
    );
    Ok(())
}

#[test]
fn makefile_declares_the_polonius_flags_variable() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    ensure!(
        makefile
            .lines()
            .any(|line| line.trim() == format!("POLONIUS_FLAGS ?= {POLONIUS_FLAG}")),
        "the Makefile should default POLONIUS_FLAGS to {POLONIUS_FLAG}"
    );
    Ok(())
}

#[rstest]
#[case::test_nextest("test-nextest")]
#[case::doctest("doctest")]
#[case::typecheck("typecheck")]
#[case::lint_clippy("lint-clippy")]
#[case::lint_whitaker("lint-whitaker")]
#[case::build_binary("target/%/$(APP)")]
fn rustflags_setting_recipes_restate_polonius(#[case] target: &str) -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let recipe = target_recipe(&makefile, target)
        .with_context(|| format!("the Makefile should declare a {target} target"))?;
    let rustflags_lines: Vec<&str> = recipe
        .lines()
        .filter(|line| line.contains("RUSTFLAGS="))
        .collect();
    ensure!(
        !rustflags_lines.is_empty(),
        "{target} should set RUSTFLAGS, found {recipe:?}"
    );
    for line in rustflags_lines {
        ensure!(
            line.contains(POLONIUS_VAR),
            "{target} sets RUSTFLAGS without {POLONIUS_VAR}: {line:?}"
        );
    }
    Ok(())
}

#[rstest]
#[case::ci(".github/workflows/ci.yml", "build-test", true)]
#[case::netsukefile(".github/workflows/netsukefile-test.yml", "netsukefile", true)]
#[case::coverage(".github/workflows/coverage-main.yml", "coverage-upload", false)]
#[case::packaging(".github/workflows/build-and-package.yml", "build", false)]
fn workflows_preset_polonius_rustflags(
    #[case] path: &str,
    #[case] job: &str,
    #[case] pins_toolchain_env: bool,
) -> Result<()> {
    let workflow: YamlValue = serde_yaml::from_str(&read_repo_file(Utf8Path::new(path))?)
        .with_context(|| format!("parse {path}"))?;
    let rustflags = yaml_str(&workflow, &["jobs", job, "env", "RUSTFLAGS"])
        .with_context(|| format!("{path} job {job} should preset RUSTFLAGS"))?;
    ensure!(
        rustflags.contains(POLONIUS_FLAG),
        "{path} job {job} presets RUSTFLAGS without {POLONIUS_FLAG}: {rustflags:?}"
    );
    if pins_toolchain_env {
        let expected = pinned_toolchain()?;
        let toolchain = yaml_str(&workflow, &["jobs", job, "env", "NETSUKE_RUST_TOOLCHAIN"])
            .with_context(|| format!("{path} job {job} should pin NETSUKE_RUST_TOOLCHAIN"))?;
        ensure!(
            toolchain == expected,
            "{path} job {job} pins {toolchain:?}, but rust-toolchain.toml pins {expected:?}"
        );
    }
    Ok(())
}

#[test]
fn coverage_workflow_setup_matches_the_pinned_toolchain() -> Result<()> {
    let path = ".github/workflows/coverage-main.yml";
    let workflow: YamlValue = serde_yaml::from_str(&read_repo_file(Utf8Path::new(path))?)
        .with_context(|| format!("parse {path}"))?;
    let steps = workflow
        .get("jobs")
        .and_then(|jobs| jobs.get("coverage-upload"))
        .and_then(|job| job.get("steps"))
        .and_then(YamlValue::as_sequence)
        .with_context(|| format!("{path} should declare coverage-upload steps"))?;
    let setup = steps
        .iter()
        .find(|step| yaml_str(step, &["name"]) == Some("Setup Rust"))
        .with_context(|| format!("{path} should include a Setup Rust step"))?;
    let toolchain = yaml_str(setup, &["with", "toolchain"])
        .with_context(|| format!("{path} Setup Rust should request a toolchain"))?;
    let expected = pinned_toolchain()?;
    ensure!(
        toolchain == expected,
        "{path} requests {toolchain:?}, but rust-toolchain.toml pins {expected:?}"
    );
    Ok(())
}
