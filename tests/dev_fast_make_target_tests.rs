//! Behavioural tests for hermetic `dev-fast` Make target contracts.
//!
//! A fake Cargo verifies recipes: what each target selects, what it forwards,
//! and how it propagates failure. Two clusters with subjects of their own live
//! beside this file — the capability gate in [`capability_gate`], and the Cargo
//! fragment in [`cargo_fragment`].
#![cfg(all(unix, target_os = "linux"))]

use anyhow::{Result, ensure};
use rstest::{fixture, rstest};
use test_support::dev_fast::{
    BuildScenario, DEV_FAST_CONFIG_PATH, FakeRelease, MakeInvocation, Sandbox, combined,
    pinned_toolchain,
};

#[path = "dev_fast_make_target_tests/capability_gate.rs"]
mod capability_gate;

#[path = "dev_fast_make_target_tests/cargo_fragment.rs"]
mod cargo_fragment;

/// The version the fake release is published as; see the installer tests.
const TEST_MOLD_VERSION: &str = "9.9.9";
/// What a given build target must ask Cargo to do.
#[derive(Copy, Clone, Debug)]
struct BuildTarget {
    name: &'static str,
    subcommand: &'static [&'static str],
}
/// Prepare a hermetic dev-fast scenario for a Make target contract test.
#[fixture]
fn prepared_build_scenario() -> Result<BuildScenario> {
    BuildScenario::prepare()
}
#[rstest]
#[case::dev_build(BuildTarget {
    name: "dev-build",
    subcommand: &["build", "--bin", "netsuke"],
})]
#[case::dev_test(
    BuildTarget {
        name: "dev-test",
        // Mirrors `make test-nextest`, so the accelerated loop and the gate run
        // the same runner under the same `.config/nextest.toml`.
        subcommand: &[
            "nextest",
            "run",
            "--workspace",
            "--all-targets",
            "--all-features",
        ],
    }
)]
fn build_targets_select_the_pinned_toolchain_and_fragment(
    #[case] target: BuildTarget,
    #[from(prepared_build_scenario)] scenario_res: Result<BuildScenario>,
) -> Result<()> {
    let scenario = scenario_res?;
    let invocation = scenario.run(target.name)?;
    ensure!(
        invocation.toolchain() == pinned_toolchain()?,
        "`{}` should select the pinned nightly, got `{}`",
        target.name,
        invocation.toolchain()
    );
    ensure!(
        invocation.contains_sequence(&["--config", DEV_FAST_CONFIG_PATH]),
        "`{}` should pass the fragment, got `{:?}`",
        target.name,
        invocation.arguments()
    );
    ensure!(
        invocation.contains_sequence(target.subcommand),
        "`{}` should run `{:?}`, got `{:?}`",
        target.name,
        target.subcommand,
        invocation.arguments()
    );
    ensure!(
        !invocation
            .arguments()
            .iter()
            .any(|argument| argument == "--locked"),
        "`{}` should leave lockfile verification disabled by default, got `{:?}`",
        target.name,
        invocation.arguments()
    );
    // The linker is resolved by PATH order, so leading the prefix is the
    // whole mechanism by which the pinned mold, and not a system one, gets
    // used.
    ensure!(
        invocation.path_starts_with(&scenario.prefix_bin()),
        "`{}` should lead PATH with the install prefix, got `{}`",
        target.name,
        invocation.path()
    );
    Ok(())
}

#[rstest]
#[case::dev_build("dev-build", &["--locked", "--config"])]
#[case::dev_test("dev-test", &["nextest", "run", "--locked"])]
fn build_targets_forward_config_and_lockfile_overrides(
    #[case] target: &str,
    #[case] lockfile_arguments: &[&str],
    #[from(prepared_build_scenario)] scenario_res: Result<BuildScenario>,
) -> Result<()> {
    let scenario = scenario_res?;
    let config = "tools/dev-fast/config.local.toml";
    let invocation = MakeInvocation::new(target)
        .variable("CARGO", scenario.cargo().executable())
        .variable("CARGO_LOCKED", "--locked")
        .variable("DEV_FAST_CONFIG", config);
    let output = scenario.sandbox().run_make(&invocation)?;

    ensure!(
        output.status.success(),
        "make {target} should succeed, got `{}`",
        combined(&output)
    );
    let recorded = scenario.cargo().sole_invocation()?;
    ensure!(
        recorded.contains_sequence(lockfile_arguments),
        "{target} should place CARGO_LOCKED as `{:?}`, got `{:?}`",
        lockfile_arguments,
        recorded.arguments()
    );
    ensure!(
        recorded.contains_sequence(&["--config", config]),
        "{target} should forward DEV_FAST_CONFIG, got `{:?}`",
        recorded.arguments()
    );
    Ok(())
}

/// The two nextest worker bounds travel as far as Cargo's argument vector.
///
/// `test-nextest` is exercised by CI, so a bound dropped there fails loudly;
/// `dev-test` is a local loop no lane runs, so nothing else would notice one
/// going missing. Asserting on the recorded invocation rather than on recipe
/// text is what makes the agreement between the two targets a fact rather than
/// a reading: a variable the recipe fails to expand reaches Cargo as a literal
/// `$(...)` argument, which the count below rejects and a recipe-level
/// substring check would accept.
///
/// The empty case is the control. With nothing set the recipe must contribute
/// no bound of its own, so every bound observed in the other cases came from
/// the caller's variable.
#[rstest]
#[case::no_bounds(&[])]
#[case::build_jobs_only(&[("NEXTEST_BUILD_JOBS", "--build-jobs 4")])]
#[case::test_jobs_only(&[("NEXTEST_TEST_JOBS", "-j 4")])]
#[case::both_bounds(&[("NEXTEST_BUILD_JOBS", "--build-jobs 4"), ("NEXTEST_TEST_JOBS", "-j 4")])]
fn dev_test_forwards_the_nextest_worker_bounds(
    #[case] bounds: &[(&str, &str)],
    #[from(prepared_build_scenario)] scenario_res: Result<BuildScenario>,
) -> Result<()> {
    let scenario = scenario_res?;
    let mut invocation =
        MakeInvocation::new("dev-test").variable("CARGO", scenario.cargo().executable());
    for (name, value) in bounds {
        invocation = invocation.variable(name, value);
    }
    let output = scenario.sandbox().run_make(&invocation)?;

    ensure!(
        output.status.success(),
        "make dev-test should succeed, got `{}`",
        combined(&output)
    );
    let recorded = scenario.cargo().sole_invocation()?;
    for (name, value) in bounds {
        ensure!(
            recorded.contains_sequence(&value.split_whitespace().collect::<Vec<_>>()),
            "dev-test should forward {name}={value}, got `{:?}`",
            recorded.arguments()
        );
    }
    // Each bound the caller sets contributes exactly one flag here, so matching
    // the count proves the recipe neither drops one nor invents a bound of its
    // own to replace it.
    let bound_flags = recorded
        .arguments()
        .iter()
        .filter(|argument| matches!(argument.as_str(), "--build-jobs" | "-j"))
        .count();
    ensure!(
        bound_flags == bounds.len(),
        "dev-test should forward exactly the {} bound(s) the caller set, got `{:?}`",
        bounds.len(),
        recorded.arguments()
    );
    Ok(())
}

#[rstest]
#[case("dev-build")]
#[case("dev-test")]
fn build_targets_do_not_evaluate_a_config_override(
    #[case] target: &str,
    #[from(prepared_build_scenario)] scenario_res: Result<BuildScenario>,
) -> Result<()> {
    let scenario = scenario_res?;
    let marker = scenario.sandbox().home().join("config-evaluation-marker");
    let config = format!("`touch {marker}`");
    let invocation = MakeInvocation::new(target)
        .variable("CARGO", scenario.cargo().executable())
        .variable("DEV_FAST_CONFIG", config);
    let output = scenario.sandbox().run_make(&invocation)?;

    ensure!(
        output.status.success(),
        "make {target} should treat the override as data, got `{}`",
        combined(&output)
    );
    ensure!(
        !marker.as_std_path().exists(),
        "make {target} must not evaluate DEV_FAST_CONFIG as shell syntax"
    );
    Ok(())
}

#[rstest]
#[case("dev-build")]
#[case("dev-test")]
fn build_targets_propagate_cargo_failure(
    #[case] target: &str,
    #[from(prepared_build_scenario)] scenario_res: Result<BuildScenario>,
) -> Result<()> {
    let scenario = scenario_res?;
    let marker = scenario
        .sandbox()
        .home()
        .join(format!("failing-cargo-{target}.marker"));
    let cargo = scenario.sandbox().write_fake(
        &scenario.sandbox().bin(),
        "failing-cargo",
        &format!(": > \"{marker}\"\nexit 17"),
    )?;
    let invocation = MakeInvocation::new(target).variable("CARGO", cargo);
    let output = scenario.sandbox().run_make(&invocation)?;

    ensure!(
        marker.as_std_path().exists(),
        "fake Cargo must run before make {target} propagates its failure"
    );
    ensure!(
        output.status.code() == Some(2),
        "make {target} should report Cargo exit status 17 as Make failure 2, got `{:?}`",
        output.status.code()
    );
    ensure!(
        !output.status.success(),
        "make {target} must fail when Cargo exits unsuccessfully"
    );
    Ok(())
}

#[test]
fn install_target_forwards_the_prefix_pins_and_release_url() -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    let release = FakeRelease::publish(&sandbox, TEST_MOLD_VERSION)?;
    let version_pin = release.write_version_pin(&sandbox)?;
    let checksums = release.write_checksums(&sandbox, release.sha256())?;

    // Pins go through as command-line variables, outranking the Makefile's `?=`
    // defaults; the release URL is read straight from the environment by the
    // script, which is the only channel available for it.
    let invocation = MakeInvocation::new("install-dev-fast")
        .variable("MOLD_VERSION_FILE", &version_pin)
        .variable("MOLD_SHA256SUMS_FILE", &checksums)
        .environment("MOLD_RELEASE_BASE_URL", release.base_url());
    let output = sandbox.run_make(&invocation)?;
    let text = combined(&output);

    ensure!(
        output.status.success(),
        "make install-dev-fast should succeed, got `{text}`"
    );
    ensure!(
        text.contains(&release.base_url()),
        "should fetch from the local release URL, got `{text}`"
    );
    ensure!(
        text.contains(&format!("verified {}", release.name())),
        "should verify against the overridden checksum file, got `{text}`"
    );
    ensure!(
        text.contains(sandbox.prefix().as_str()),
        "should install into the forwarded prefix, got `{text}`"
    );
    ensure!(
        sandbox.prefix().join("bin/mold").as_std_path().is_file(),
        "the pinned linker should land in the forwarded prefix"
    );
    Ok(())
}
