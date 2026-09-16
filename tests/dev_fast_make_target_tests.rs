//! Behavioural tests for the hermetic build-standard Make target contracts.
//!
//! A fake Cargo verifies the recipes; real Cargo resolves the committed
//! configuration in its own test.
#![cfg(all(unix, target_os = "linux"))]

use anyhow::{Context, Result, ensure};
use rstest::{fixture, rstest};
use test_support::dev_fast::{
    BuildScenario, CARGO_CONFIG_PATH, CargoInvocation, MakeInvocation, RecordingCargo, Sandbox,
    cargo_config, combined, pinned_mold_version, pinned_toolchain,
};

/// A binary name no build tree can already contain, so the file rule's recipe
/// runs instead of being skipped as up to date on a developer's machine.
const PROBE_APP: &str = "netsuke-make-contract-probe";
/// What a given target must ask Cargo to do.
#[derive(Copy, Clone, Debug)]
struct BuildTarget {
    name: &'static str,
    subcommand: &'static [&'static str],
}

/// Prepare a hermetic scenario for a Make target contract test.
#[fixture]
fn prepared_build_scenario() -> Result<BuildScenario> {
    BuildScenario::prepare()
}

/// Run `target` under `scenario` with the probe binary name, returning every
/// Cargo invocation it produced.
fn run_target(scenario: &BuildScenario, target: &str) -> Result<Vec<CargoInvocation>> {
    let invocation = MakeInvocation::new(target)
        .variable("CARGO", scenario.cargo().executable())
        .variable("APP", PROBE_APP);
    let output = scenario.sandbox().run_make(&invocation)?;
    ensure!(
        output.status.success(),
        "make {target} should succeed, got `{}`",
        combined(&output)
    );
    let recorded = scenario.cargo().invocations()?;
    ensure!(
        !recorded.is_empty(),
        "make {target} should invoke Cargo at least once"
    );
    Ok(recorded)
}

/// The flags the standard applies on this platform, read from the committed
/// configuration so the test cannot restate the Makefile's own answer.
fn standard_flags() -> Result<Vec<String>> {
    let config: toml::Value = toml::from_str(&cargo_config()?)?;
    let flags = config
        .get("target")
        .and_then(|table| table.get(r#"cfg(target_os = "linux")"#))
        .and_then(|table| table.get("rustflags"))
        .and_then(toml::Value::as_array)
        .context("the configuration must gate rustflags behind the Linux cfg")?;
    Ok(flags
        .iter()
        .filter_map(toml::Value::as_str)
        .map(str::to_owned)
        .collect())
}

/// Every gate target must hand Cargo the standard's flags through `RUSTFLAGS`.
///
/// This is the whole point of restating them in the Makefile. Each of these
/// recipes assigns `RUSTFLAGS` to deny warnings, and an assigned `RUSTFLAGS`
/// displaces every `rustflags` table in `.cargo/config.toml`, so a recipe that
/// forgot to restate them would link with the platform linker and a
/// single-threaded frontend while still reporting success.
#[rstest]
#[case::test_nextest(BuildTarget {
    name: "test-nextest",
    subcommand: &["nextest", "run", "--workspace", "--all-targets", "--all-features"],
})]
#[case::doctest(BuildTarget {
    name: "doctest",
    subcommand: &["test", "--workspace", "--doc", "--all-features"],
})]
#[case::lint_clippy(BuildTarget { name: "lint-clippy", subcommand: &["clippy"] })]
fn gate_targets_deny_warnings_and_apply_the_standard(
    #[case] target: BuildTarget,
    #[from(prepared_build_scenario)] scenario_res: Result<BuildScenario>,
) -> Result<()> {
    let scenario = scenario_res?;
    let recorded = run_target(&scenario, target.name)?;
    let flags = standard_flags()?;
    let borrowed: Vec<&str> = flags.iter().map(String::as_str).collect();

    for invocation in &recorded {
        ensure!(
            invocation.rustflags_contain(&["-D", "warnings"]),
            "`{}` should deny warnings, got `{:?}`",
            target.name,
            invocation.rustflags()
        );
        ensure!(
            invocation.rustflags_contain(&borrowed),
            "`{}` should apply {:?}, got `{:?}`",
            target.name,
            flags,
            invocation.rustflags()
        );
        // The linker is resolved by PATH order, so leading the prefix is the
        // whole mechanism by which the pinned mold, and not a system one, is
        // the one `-fuse-ld=mold` finds.
        ensure!(
            invocation.path_starts_with(&scenario.prefix_bin()),
            "`{}` should lead PATH with the install prefix, got `{}`",
            target.name,
            invocation.path()
        );
    }
    ensure!(
        recorded
            .iter()
            .any(|invocation| invocation.contains_sequence(target.subcommand)),
        "`{}` should run `{:?}`",
        target.name,
        target.subcommand
    );
    Ok(())
}

/// The debug build takes the standard but not the gates' warning policy.
///
/// Asserting the absence matters: folding `-D warnings` into the shared
/// composition would silently turn every `make build` into a gate, which is a
/// different contract from the one this target has always had.
#[rstest]
fn the_debug_build_applies_the_standard_without_denying_warnings(
    #[from(prepared_build_scenario)] scenario_res: Result<BuildScenario>,
) -> Result<()> {
    let scenario = scenario_res?;
    let recorded = run_target(&scenario, "build")?;
    let invocation = recorded.first().context("build should invoke Cargo")?;
    let flags = standard_flags()?;
    let borrowed: Vec<&str> = flags.iter().map(String::as_str).collect();

    ensure!(
        invocation.rustflags_contain(&borrowed),
        "build should apply {flags:?}, got `{:?}`",
        invocation.rustflags()
    );
    ensure!(
        !invocation.rustflags_contain(&["-D", "warnings"]),
        "build should not impose the gates' warning policy, got `{:?}`",
        invocation.rustflags()
    );
    ensure!(
        invocation.contains_sequence(&["build", "--bin", PROBE_APP]),
        "build should build the binary, got `{:?}`",
        invocation.arguments()
    );
    Ok(())
}

/// Release builds are one of the two exclusions, and the exclusion works by
/// assignment rather than by content: assigning `RUSTFLAGS` at all is what
/// displaces the configuration file's tables. A recipe that left the variable
/// unset would inherit the standard from the configuration and ship an
/// artefact built with the parallel frontend and `mold`.
#[rstest]
fn the_release_build_assigns_rustflags_and_takes_neither_flag(
    #[from(prepared_build_scenario)] scenario_res: Result<BuildScenario>,
) -> Result<()> {
    let scenario = scenario_res?;
    let recorded = run_target(&scenario, "release")?;
    let invocation = recorded.first().context("release should invoke Cargo")?;

    ensure!(
        invocation.rustflags().is_some(),
        "release must assign RUSTFLAGS, or the configuration's tables apply"
    );
    for flag in standard_flags()? {
        ensure!(
            !invocation.rustflags_contain(&[flag.as_str()]),
            "release must not carry `{flag}`, got `{:?}`",
            invocation.rustflags()
        );
    }
    ensure!(
        invocation.contains_sequence(&["--release", "--bin", PROBE_APP]),
        "release should build the release binary, got `{:?}`",
        invocation.arguments()
    );
    Ok(())
}

/// Every standard flag a gate passes must be one the committed configuration
/// names, and vice versa.
///
/// The two directions fail to different mutations, which is why both are here:
/// dropping a flag from the Makefile fails the forward check in the tests
/// above, while dropping one from the configuration — leaving a bare `cargo
/// build` without it — fails only this reverse check.
#[rstest]
fn the_makefile_passes_no_standard_flag_the_configuration_omits(
    #[from(prepared_build_scenario)] scenario_res: Result<BuildScenario>,
) -> Result<()> {
    let scenario = scenario_res?;
    let recorded = run_target(&scenario, "build")?;
    let invocation = recorded.first().context("build should invoke Cargo")?;
    let configured = standard_flags()?;
    let passed = invocation.rustflags().unwrap_or_default();

    for flag in passed.split_whitespace() {
        ensure!(
            configured.iter().any(|known| known == flag),
            "`{flag}` reaches Cargo from the Makefile but is absent from {CARGO_CONFIG_PATH}, \
             so a bare `cargo build` would not get it"
        );
    }
    Ok(())
}

#[rstest]
#[case::dev_build("dev-build")]
#[case::dev_test("dev-test")]
fn deprecated_aliases_still_reach_cargo(
    #[case] target: &str,
    #[from(prepared_build_scenario)] scenario_res: Result<BuildScenario>,
) -> Result<()> {
    let scenario = scenario_res?;
    let recorded = run_target(&scenario, target)?;
    let flags = standard_flags()?;
    let borrowed: Vec<&str> = flags.iter().map(String::as_str).collect();

    ensure!(
        recorded
            .iter()
            .all(|invocation| invocation.rustflags_contain(&borrowed)),
        "`{target}` should apply the same standard as the target it aliases"
    );
    Ok(())
}

/// The capability check gates the build targets, so a failing check must stop
/// them before Cargo runs. Asserting on the recorded invocations proves that
/// directly, where relying on Cargo's absence from the sandbox would pass even
/// if the recipe did invoke it.
#[rstest]
#[case("build")]
#[case("test-nextest")]
#[case("doctest")]
#[case("lint-clippy")]
fn a_failed_gate_invokes_cargo_not_at_all(#[case] target: &str) -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    // A usable cargo is present and recording; only mold is missing.
    let cargo = RecordingCargo::install(&sandbox)?;

    let invocation = MakeInvocation::new(target)
        .variable("CARGO", cargo.executable())
        .variable("APP", PROBE_APP);
    let output = sandbox.run_make(&invocation)?;
    let text = combined(&output);

    ensure!(
        !output.status.success(),
        "`{target}` should fail, got `{text}`"
    );
    ensure!(
        text.contains("mold not found on PATH"),
        "`{target}` should fail in the capability check, got `{text}`"
    );
    let recorded = cargo.invocations()?;
    ensure!(
        recorded.is_empty(),
        "`{target}` should not reach Cargo, recorded {} invocation(s)",
        recorded.len()
    );
    Ok(())
}

/// A drifting `mold` fails the gate, so it must stop the build targets exactly
/// as a missing one does.
///
/// Worth asserting separately from the absent case: a drift leaves a perfectly
/// usable linker on `PATH`, so nothing but the gate's own verdict prevents the
/// recipe from proceeding.
#[rstest]
#[case("build")]
#[case("test-nextest")]
fn a_drifting_mold_invokes_cargo_not_at_all(#[case] target: &str) -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    sandbox.write_mold(&sandbox.prefix().join("bin"), "99.0.0")?;
    let cargo = RecordingCargo::install(&sandbox)?;

    let invocation = MakeInvocation::new(target)
        .variable("CARGO", cargo.executable())
        .variable("APP", PROBE_APP);
    let output = sandbox.run_make(&invocation)?;
    let text = combined(&output);

    ensure!(
        !output.status.success(),
        "`{target}` should fail on a drifting mold, got `{text}`"
    );
    // Pin the failure to its cause. Asserting only the exit status would let
    // this pass on an unrelated failure — a missing `rustup`, or a broken
    // recipe — and so would stop testing the drift gate at all.
    ensure!(
        text.contains("does not match the pin") && text.contains(&pinned_mold_version()?),
        "`{target}` should name the version mismatch and the pin, got `{text}`"
    );
    let recorded = cargo.invocations()?;
    ensure!(
        recorded.is_empty(),
        "`{target}` should not reach Cargo, recorded {} invocation(s)",
        recorded.len()
    );
    Ok(())
}

/// The release build is deliberately *not* gated: it uses neither Cranelift nor
/// `mold`, so requiring them would make packaging depend on tools it never
/// invokes.
#[test]
fn the_release_build_runs_without_the_capability_check() -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    let cargo = RecordingCargo::install(&sandbox)?;

    let invocation = MakeInvocation::new("release")
        .variable("CARGO", cargo.executable())
        .variable("APP", PROBE_APP);
    let output = sandbox.run_make(&invocation)?;

    ensure!(
        output.status.success(),
        "make release should not require mold, got `{}`",
        combined(&output)
    );
    ensure!(
        cargo.invocations()?.len() == 1,
        "make release should invoke Cargo exactly once"
    );
    Ok(())
}

#[rstest]
#[case("build")]
#[case("test-nextest")]
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
    let invocation = MakeInvocation::new(target)
        .variable("CARGO", cargo)
        .variable("APP", PROBE_APP);
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
    Ok(())
}
