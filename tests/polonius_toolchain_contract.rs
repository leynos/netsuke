//! Contract tests pinning the Polonius toolchain plumbing.
//!
//! The tree only borrow-checks under the Polonius alpha analysis (see ADR-0006
//! and docs/polonius.md). Nightly toolchains dated 2026-08-04 and later enable
//! Polonius by default, so the requirement is carried entirely by the dated
//! pin in `rust-toolchain.toml`: no build configuration passes a `-Zpolonius`
//! directive any more, and reintroducing one would pin the tree to an
//! interface that is on its way out. These tests hold both halves — the pin is
//! recent enough to enable Polonius, and nothing restates the retired flag —
//! plus the workflow contract that keeps CI on the same pinned channel.
//!
//! The assertions that describe how one job obtains its channel live in
//! `tests/polonius_toolchain_contract/toolchain_wiring.rs`, which keeps this
//! file inside the repository's 400-line limit.

#[path = "support/makefile.rs"]
mod makefile;
#[path = "support/shared_actions.rs"]
pub mod shared_actions;
#[path = "polonius_toolchain_contract/toolchain_wiring.rs"]
mod toolchain_wiring;

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
const ENV_TOOLCHAIN_INPUT: &str = "${{ env.NETSUKE_RUST_TOOLCHAIN }}";
/// The composite action that exports `rust-toolchain.toml`'s channel.
const RESOLVE_TOOLCHAIN_ACTION: &str = "./.github/actions/resolve-toolchain";

/// Where a workflow obtains the channel it provisions.
#[derive(Clone, Copy)]
enum ToolchainPin {
    /// The job runs [`RESOLVE_TOOLCHAIN_ACTION`] before the shared action, which
    /// exports `NETSUKE_RUST_TOOLCHAIN`, and passes [`ENV_TOOLCHAIN_INPUT`] to
    /// the shared action. The job declares no channel of its own.
    FromFile,
    /// The job neither declares nor reads the pin.
    Absent,
}

/// Build-configuration surfaces that could reintroduce the retired directive.
///
/// `.cargo/config.toml` is the path Cargo auto-discovers, so it is the surface
/// on which the directive would reach every build at once. It is committed by
/// the build standard, so it is required like every other path here rather than
/// skipped when absent: a file that has gone missing would otherwise take its
/// flags out of this contract silently, which is the regression the contract
/// exists to catch.
const BUILD_CONFIGURATION_FILES: [&str; 7] = [
    "Makefile",
    ".cargo/config.toml",
    "scripts/build-tools-common.sh",
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
    ///
    /// `None` is a requirement that the input be absent, not the absence of a
    /// requirement: this field is compared for equality like the others.
    rustflags: Option<&'static str>,
    /// How the job obtains the channel it provisions.
    ///
    /// Each arm is a requirement compared for equality, so an unexpected pin
    /// shape fails rather than being read as "no pin required".
    pin: ToolchainPin,
}

const CI_WORKFLOW: WorkflowExpectation = WorkflowExpectation {
    path: ".github/workflows/ci.yml",
    job: "build-test",
    action: SETUP_RUST_ACTION,
    rustflags: Some(DENY_WARNINGS_RUSTFLAGS),
    pin: ToolchainPin::FromFile,
};
/// The second `ci.yml` job on the shared-action contract.
///
/// It passes no `rustflags`, so `None` here means the action's default is
/// inherited rather than denied, the same shape as the coverage job.
const CI_KANI_SMOKE_WORKFLOW: WorkflowExpectation = WorkflowExpectation {
    path: ".github/workflows/ci.yml",
    job: "kani-smoke",
    action: SETUP_RUST_ACTION,
    rustflags: None,
    pin: ToolchainPin::FromFile,
};
const CI_WINDOWS_WORKFLOW: WorkflowExpectation = WorkflowExpectation {
    path: ".github/workflows/ci-windows.yml",
    job: "build-test-windows",
    action: SETUP_RUST_ACTION,
    rustflags: Some(DENY_WARNINGS_RUSTFLAGS),
    pin: ToolchainPin::FromFile,
};
/// The default-feature lane, a reusable workflow that resolves the channel
/// from `rust-toolchain.toml` itself rather than receiving it from the caller.
const CI_DEFAULT_FEATURES_WORKFLOW: WorkflowExpectation = WorkflowExpectation {
    path: ".github/workflows/ci-default-features.yml",
    job: "default-features",
    action: SETUP_RUST_ACTION,
    rustflags: Some(DENY_WARNINGS_RUSTFLAGS),
    pin: ToolchainPin::FromFile,
};
const NETSUKEFILE_WORKFLOW: WorkflowExpectation = WorkflowExpectation {
    path: ".github/workflows/netsukefile-test.yml",
    job: "netsukefile",
    action: SETUP_RUST_ACTION,
    rustflags: None,
    pin: ToolchainPin::FromFile,
};
const COVERAGE_WORKFLOW: WorkflowExpectation = WorkflowExpectation {
    path: ".github/workflows/coverage-main.yml",
    job: "coverage-upload",
    action: SETUP_RUST_ACTION,
    rustflags: Some(DENY_WARNINGS_RUSTFLAGS),
    pin: ToolchainPin::FromFile,
};
const PACKAGING_WORKFLOW: WorkflowExpectation = WorkflowExpectation {
    path: ".github/workflows/build-and-package.yml",
    job: "build",
    action: RUST_BUILD_RELEASE_ACTION,
    rustflags: None,
    pin: ToolchainPin::Absent,
};

/// Every workflow under the shared-action toolchain contract.
const WORKFLOW_EXPECTATIONS: [WorkflowExpectation; 7] = [
    CI_WORKFLOW,
    CI_KANI_SMOKE_WORKFLOW,
    CI_WINDOWS_WORKFLOW,
    CI_DEFAULT_FEATURES_WORKFLOW,
    NETSUKEFILE_WORKFLOW,
    COVERAGE_WORKFLOW,
    PACKAGING_WORKFLOW,
];

/// Returns the single shared-actions commit SHA the checked toolchain actions
/// pin.
///
/// The SHA's value is owned by the workflow files (and the pin-bump process
/// that updates them); this contract derives it rather than restating it, so
/// a complete bump stays green while a partial bump — some toolchain actions
/// moved, others left behind — fails on the disagreement. Other shared
/// actions may use independent pins; the broader shape-only sweep across every
/// workflow lives in `workflow_shared_actions_pins`.
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
        let expected_prefix = format!("{}@", expectation.action);
        refs.extend(
            extracted
                .into_iter()
                .filter(|reference| reference.starts_with(&expected_prefix)),
        );
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
        let contents = root
            .read_to_string(path)
            .with_context(|| format!("read {path}"))?;
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
#[case::ci_kani_smoke(CI_KANI_SMOKE_WORKFLOW)]
#[case::ci_default_features(CI_DEFAULT_FEATURES_WORKFLOW)]
#[case::ci_windows(CI_WINDOWS_WORKFLOW)]
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
        pin,
    } = expectation;
    let named = toolchain_wiring::JobRef { path, job };
    let workflow: YamlValue = serde_yaml::from_str(&read_repo_file(Utf8Path::new(path))?)
        .with_context(|| format!("parse {path}"))?;
    ensure!(
        yaml_str(&workflow, &["jobs", job, "env", "RUSTFLAGS"]).is_none(),
        "{named} should pass RUSTFLAGS through the shared action input"
    );
    let steps = workflow
        .get("jobs")
        .and_then(|jobs| jobs.get(job))
        .and_then(|job_value| job_value.get("steps"))
        .and_then(YamlValue::as_sequence)
        .with_context(|| format!("{named} should declare steps"))?;
    let expected_action = &format!("{action}@{}", shared_actions_sha()?);
    let shared_action = toolchain_wiring::find_shared_action(steps, named, expected_action)?;
    let rustflags = yaml_str(shared_action, &["with", "rustflags"]);
    ensure!(
        rustflags == expected_rustflags,
        "{path} {expected_action} passes {rustflags:?}, expected {expected_rustflags:?}"
    );
    toolchain_wiring::ensure_toolchain_pin(
        &toolchain_wiring::ToolchainWiring {
            named,
            toolchain_env: yaml_str(&workflow, &["jobs", job, "env", "NETSUKE_RUST_TOOLCHAIN"]),
            toolchain_input: yaml_str(shared_action, &["with", "toolchain"]),
            steps,
            shared_action,
        },
        pin,
    )
}

#[test]
fn resolve_toolchain_action_exports_the_channel_from_the_toolchain_file() -> Result<()> {
    let path = ".github/actions/resolve-toolchain/action.yml";
    let action: YamlValue = serde_yaml::from_str(&read_repo_file(Utf8Path::new(path))?)
        .with_context(|| format!("parse {path}"))?;
    let steps = action
        .get("runs")
        .and_then(|runs| runs.get("steps"))
        .and_then(YamlValue::as_sequence)
        .with_context(|| format!("{path} should declare composite steps"))?;
    let script = steps
        .iter()
        .find_map(|step| yaml_str(step, &["run"]))
        .with_context(|| format!("{path} should run a script"))?;
    // Run the real script against throwaway toolchain files and read what it
    // exports, rather than matching its text.
    for channel in ["nightly-2030-01-02", "nightly-2031-03-04"] {
        let exported = run_resolver(script, &format!("[toolchain]\nchannel = \"{channel}\"\n"))?
            .with_context(|| format!("{path} should succeed for {channel}"))?;
        ensure!(
            exported == format!("NETSUKE_RUST_TOOLCHAIN={channel}\n"),
            "{path} exported {exported:?} for {channel}"
        );
    }
    for broken in [
        "",
        "[toolchain]\ncomponents = []\n",
        "[toolchain]\nchannel = \"\"\n",
    ] {
        ensure!(
            run_resolver(script, broken)?.is_none(),
            "{path} should fail, exporting nothing, for {broken:?}"
        );
    }
    Ok(())
}

/// Runs the resolver script in a scratch directory holding `toolchain` as
/// `rust-toolchain.toml`. Returns what it appended to `GITHUB_ENV` on success
/// and `None` when it failed.
fn run_resolver(script: &str, toolchain: &str) -> Result<Option<String>> {
    let scratch = tempfile::tempdir().context("create a scratch directory")?;
    let dir = cap_std::fs_utf8::Dir::open_ambient_dir(
        Utf8Path::from_path(scratch.path()).context("scratch path should be UTF-8")?,
        cap_std::ambient_authority(),
    )
    .context("open the scratch directory")?;
    dir.write("rust-toolchain.toml", toolchain)?;
    dir.write("github-env", "")?;
    let status = std::process::Command::new("bash")
        .arg("-c")
        .arg(script)
        .current_dir(scratch.path())
        .env("GITHUB_ENV", scratch.path().join("github-env"))
        .status()
        .context("run the resolver with bash")?;
    Ok(status
        .success()
        .then(|| dir.read_to_string("github-env"))
        .transpose()?)
}
