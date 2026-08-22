//! Contract tests pinning the Polonius toolchain plumbing.
//!
//! The tree only borrow-checks under the Polonius alpha analysis (see ADR-006
//! and docs/polonius.md). Nightly toolchains dated 2026-08-04 and later enable
//! Polonius by default, so the requirement is carried entirely by the dated
//! pin in `rust-toolchain.toml`: no build configuration passes a `-Zpolonius`
//! directive any more, and reintroducing one would pin the tree to an
//! interface that is on its way out. These tests hold both halves — the pin is
//! recent enough to enable Polonius, and nothing restates the retired flag —
//! plus the workflow contract that keeps CI on the same pinned channel.

#[path = "support/makefile.rs"]
mod makefile;
#[path = "support/shared_actions.rs"]
pub mod shared_actions;

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use makefile::{read_repo_file, repo_root};
use rstest::rstest;
use serde_yaml::Value as YamlValue;
use toml::Value as TomlValue;

/// The retired directive that must not reappear in build configuration.
const POLONIUS_FLAG: &str = "-Zpolonius";
/// The first nightly on which Polonius is the default borrow-check analysis.
const POLONIUS_DEFAULT_SINCE: &str = "2026-08-04";
const SETUP_RUST_ACTION: &str = "leynos/shared-actions/.github/actions/setup-rust";
const RUST_BUILD_RELEASE_ACTION: &str = "leynos/shared-actions/.github/actions/rust-build-release";
const DENY_WARNINGS_RUSTFLAGS: &str = "-D warnings";

/// Build-configuration surfaces that could reintroduce the retired directive.
///
/// `.cargo/config.toml` is listed even though it no longer exists: it is the
/// path Cargo auto-discovers, so recreating it to carry the flag is the most
/// likely regression and a missing file is simply skipped.
const BUILD_CONFIGURATION_FILES: [&str; 8] = [
    "Makefile",
    ".cargo/config.toml",
    "tools/dev-fast/config.toml",
    "scripts/dev-fast-common.sh",
    ".github/workflows/ci.yml",
    ".github/workflows/netsukefile-test.yml",
    ".github/workflows/coverage-main.yml",
    ".github/workflows/build-and-package.yml",
];

/// Describes one workflow's shared-action and toolchain contract.
struct WorkflowExpectation {
    path: &'static str,
    job: &'static str,
    action: &'static str,
    /// The `with.rustflags` input the job must pass, or `None` when it must
    /// pass none at all and inherit the action's default.
    rustflags: Option<&'static str>,
    pins_toolchain_env: bool,
}

const CI_WORKFLOW: WorkflowExpectation = WorkflowExpectation {
    path: ".github/workflows/ci.yml",
    job: "build-test",
    action: SETUP_RUST_ACTION,
    rustflags: Some(DENY_WARNINGS_RUSTFLAGS),
    pins_toolchain_env: true,
};
const NETSUKEFILE_WORKFLOW: WorkflowExpectation = WorkflowExpectation {
    path: ".github/workflows/netsukefile-test.yml",
    job: "netsukefile",
    action: SETUP_RUST_ACTION,
    rustflags: None,
    pins_toolchain_env: true,
};
const COVERAGE_WORKFLOW: WorkflowExpectation = WorkflowExpectation {
    path: ".github/workflows/coverage-main.yml",
    job: "coverage-upload",
    action: SETUP_RUST_ACTION,
    rustflags: Some(DENY_WARNINGS_RUSTFLAGS),
    pins_toolchain_env: false,
};
const PACKAGING_WORKFLOW: WorkflowExpectation = WorkflowExpectation {
    path: ".github/workflows/build-and-package.yml",
    job: "build",
    action: RUST_BUILD_RELEASE_ACTION,
    rustflags: None,
    pins_toolchain_env: false,
};

/// Every workflow under the shared-action toolchain contract.
const WORKFLOW_EXPECTATIONS: [WorkflowExpectation; 4] = [
    CI_WORKFLOW,
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
fn rust_toolchain_pins_a_nightly_that_enables_polonius_by_default() -> Result<()> {
    let channel = pinned_toolchain()?;
    let date = channel
        .strip_prefix("nightly-")
        .context("rust-toolchain.toml should pin a dated nightly")?;
    // ISO-8601 dates sort lexicographically, so a string comparison is a date
    // comparison here and needs no calendar parsing.
    ensure!(
        date >= POLONIUS_DEFAULT_SINCE,
        "the pinned nightly {channel:?} predates {POLONIUS_DEFAULT_SINCE}, \
         when Polonius became the default borrow-check analysis"
    );
    Ok(())
}

#[test]
fn build_configuration_does_not_restate_the_retired_polonius_flag() -> Result<()> {
    let root = repo_root()?;
    for path in BUILD_CONFIGURATION_FILES {
        let Ok(contents) = root.read_to_string(path) else {
            // An absent file cannot carry the flag; `.cargo/config.toml` is
            // listed precisely because it is expected to be missing.
            continue;
        };
        ensure!(
            !contents.contains(POLONIUS_FLAG),
            "{path} passes {POLONIUS_FLAG}; the pinned nightly enables Polonius by default \
             and the directive is being retired"
        );
    }
    Ok(())
}

#[rstest]
#[case::ci(CI_WORKFLOW)]
#[case::netsukefile(NETSUKEFILE_WORKFLOW)]
#[case::coverage(COVERAGE_WORKFLOW)]
#[case::packaging(PACKAGING_WORKFLOW)]
fn workflows_agree_with_the_pinned_toolchain(
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
    let rustflags = yaml_str(shared_action, &["with", "rustflags"]);
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
