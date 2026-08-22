//! Contract tests pinning the Polonius toolchain plumbing.
//!
//! The tree only borrow-checks under `-Zpolonius=next` on the dated nightly
//! pinned in `rust-toolchain.toml` (see ADR-006 and docs/polonius.md). An
//! inherited `RUSTFLAGS` environment variable overrides the
//! `.cargo/config.toml` `build.rustflags` table, so Makefile recipes that
//! compile borrow-checked targets must restate the flag, while workflows must
//! pass it through the shared action's `with.rustflags` input and reject a
//! job-level `env.RUSTFLAGS` override. These tests fail when any layer drops
//! the required policy, so a regression cannot reach CI as a confusing
//! borrow-check error.

#[path = "support/makefile.rs"]
mod makefile;
#[path = "support/shared_actions.rs"]
pub mod shared_actions;

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use makefile::{read_repo_file, target_recipe};
use rstest::rstest;
use serde_yaml::Value as YamlValue;
use toml::Value as TomlValue;

const POLONIUS_FLAG: &str = "-Zpolonius=next";
const POLONIUS_VAR: &str = "$(POLONIUS_FLAGS)";
const SETUP_RUST_ACTION: &str = "leynos/shared-actions/.github/actions/setup-rust";
const RUST_BUILD_RELEASE_ACTION: &str = "leynos/shared-actions/.github/actions/rust-build-release";
const WARNINGS_POLONIUS_RUSTFLAGS: &str = "-D warnings -Zpolonius=next";

/// Describes one workflow's shared-action and toolchain contract.
struct WorkflowExpectation {
    path: &'static str,
    job: &'static str,
    action: &'static str,
    rustflags: &'static str,
    pins_toolchain_env: bool,
}

const CI_WORKFLOW: WorkflowExpectation = WorkflowExpectation {
    path: ".github/workflows/ci.yml",
    job: "build-test",
    action: SETUP_RUST_ACTION,
    rustflags: WARNINGS_POLONIUS_RUSTFLAGS,
    pins_toolchain_env: true,
};
const CI_WINDOWS_WORKFLOW: WorkflowExpectation = WorkflowExpectation {
    path: ".github/workflows/ci.yml",
    job: "build-test-windows",
    action: SETUP_RUST_ACTION,
    rustflags: WARNINGS_POLONIUS_RUSTFLAGS,
    pins_toolchain_env: true,
};
const NETSUKEFILE_WORKFLOW: WorkflowExpectation = WorkflowExpectation {
    path: ".github/workflows/netsukefile-test.yml",
    job: "netsukefile",
    action: SETUP_RUST_ACTION,
    rustflags: POLONIUS_FLAG,
    pins_toolchain_env: true,
};
const COVERAGE_WORKFLOW: WorkflowExpectation = WorkflowExpectation {
    path: ".github/workflows/coverage-main.yml",
    job: "coverage-upload",
    action: SETUP_RUST_ACTION,
    rustflags: WARNINGS_POLONIUS_RUSTFLAGS,
    pins_toolchain_env: false,
};
const PACKAGING_WORKFLOW: WorkflowExpectation = WorkflowExpectation {
    path: ".github/workflows/build-and-package.yml",
    job: "build",
    action: RUST_BUILD_RELEASE_ACTION,
    rustflags: POLONIUS_FLAG,
    pins_toolchain_env: false,
};

/// Every workflow under the shared-action toolchain contract.
const WORKFLOW_EXPECTATIONS: [WorkflowExpectation; 5] = [
    CI_WORKFLOW,
    CI_WINDOWS_WORKFLOW,
    NETSUKEFILE_WORKFLOW,
    COVERAGE_WORKFLOW,
    PACKAGING_WORKFLOW,
];

/// Returns the single shared-actions commit SHA the checked workflows pin.
///
/// The SHA's value is owned by the workflow files (and the pin-bump process
/// that updates them); this contract derives it rather than restating it, so
/// a complete bump stays green while a partial bump — some workflows moved,
/// others left behind — fails on the disagreement. The broader shape-only
/// sweep across every workflow lives in `workflow_shared_actions_pins`.
fn shared_actions_sha() -> Result<String> {
    let mut refs = Vec::new();
    for expectation in WORKFLOW_EXPECTATIONS {
        let contents = read_repo_file(Utf8Path::new(expectation.path))?;
        let extracted =
            shared_actions::extract_shared_actions_uses(&contents).with_context(|| {
                format!("extract shared-action references from {}", expectation.path)
            })?;
        ensure!(
            !extracted.is_empty(),
            "{} should pin at least one shared action",
            expectation.path
        );
        refs.extend(extracted);
    }
    shared_actions::consistent_pin(&refs)
}

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
#[case::ci(CI_WORKFLOW)]
#[case::ci_windows(CI_WINDOWS_WORKFLOW)]
#[case::netsukefile(NETSUKEFILE_WORKFLOW)]
#[case::coverage(COVERAGE_WORKFLOW)]
#[case::packaging(PACKAGING_WORKFLOW)]
fn workflows_pass_polonius_rustflags_to_shared_actions(
    #[case] expectation: WorkflowExpectation,
) -> Result<()> {
    let WorkflowExpectation {
        path,
        job,
        action,
        rustflags: expected_rustflags,
        pins_toolchain_env,
    } = expectation;
    let expected_action = &format!("{action}@{}", shared_actions_sha()?);
    let workflow: YamlValue = serde_yaml::from_str(&read_repo_file(Utf8Path::new(path))?)
        .with_context(|| format!("parse {path}"))?;
    ensure!(
        yaml_str(&workflow, &["jobs", job, "env", "RUSTFLAGS"]).is_none(),
        "{path} job {job} should pass RUSTFLAGS through the shared action input"
    );
    let steps = workflow
        .get("jobs")
        .and_then(|jobs| jobs.get(job))
        .and_then(|job_value| job_value.get("steps"))
        .and_then(YamlValue::as_sequence)
        .with_context(|| format!("{path} job {job} should declare steps"))?;
    let shared_action = steps
        .iter()
        .find(|step| yaml_str(step, &["uses"]) == Some(expected_action.as_str()))
        .with_context(|| format!("{path} job {job} should use {expected_action}"))?;
    let rustflags = yaml_str(shared_action, &["with", "rustflags"])
        .with_context(|| format!("{path} {expected_action} should pass rustflags"))?;
    ensure!(
        rustflags == expected_rustflags,
        "{path} {expected_action} passes {rustflags:?}, expected {expected_rustflags:?}"
    );
    let toolchain_env = yaml_str(&workflow, &["jobs", job, "env", "NETSUKE_RUST_TOOLCHAIN"]);
    if pins_toolchain_env {
        let expected = pinned_toolchain()?;
        let toolchain = toolchain_env
            .with_context(|| format!("{path} job {job} should pin NETSUKE_RUST_TOOLCHAIN"))?;
        ensure!(
            toolchain == expected,
            "{path} job {job} pins {toolchain:?}, but rust-toolchain.toml pins {expected:?}"
        );
    } else {
        ensure!(
            toolchain_env.is_none(),
            "{path} job {job} should not override NETSUKE_RUST_TOOLCHAIN, found {toolchain_env:?}"
        );
    }
    Ok(())
}

#[rstest]
#[case::test_nextest("test-nextest", true)]
#[case::doctest("doctest", true)]
#[case::typecheck("typecheck", true)]
#[case::lint_clippy("lint-clippy", true)]
#[case::lint_whitaker("lint-whitaker", true)]
#[case::build_binary("target/%/$(APP)", true)]
fn rustflags_setting_recipes_apply_polonius_policy(
    #[case] target: &str,
    #[case] expects_polonius: bool,
) -> Result<()> {
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
        let contains_polonius = line.contains(POLONIUS_VAR) || line.contains(POLONIUS_FLAG);
        ensure!(
            contains_polonius == expects_polonius,
            "{target} has the wrong Polonius policy in {line:?}"
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
