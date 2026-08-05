//! Behavioural tests for the `dev-fast` Make targets' success paths.
//!
//! The scripts are covered directly elsewhere; what these tests pin down is the
//! contract the recipes themselves establish — which toolchain is selected,
//! which configuration fragment is passed, which Cargo subcommand runs, and that
//! the install prefix leads `PATH` so `-fuse-ld=mold` resolves the pinned
//! linker. A fake `cargo` records each invocation so those become checked facts
//! rather than assumptions.
//!
//! Every case is hermetic: no network, and no real mold or rustup. The
//! exception is `cargo_resolves_the_fragment_to_the_intended_settings`,
//! which runs the real Cargo via `env!("CARGO")` because only the real
//! Cargo can confirm how it resolves the `tools/dev-fast/config.toml`
//! fragment. Every other case exercises the recording fake `cargo`.

#![cfg(all(unix, target_os = "linux"))]

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use std::process::Command;
use test_support::dev_fast::{
    BuildScenario, DEV_FAST_CONFIG_PATH, FakeRelease, MakeInvocation, RecordingCargo, Sandbox,
    combined, dev_fast_config, pinned_mold_version, pinned_toolchain,
};

/// The version the fake release is published as; see the installer tests.
const TEST_MOLD_VERSION: &str = "9.9.9";

/// What a given build target must ask Cargo to do.
#[derive(Copy, Clone, Debug)]
struct BuildTarget {
    name: &'static str,
    subcommand: &'static [&'static str],
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
) -> Result<()> {
    let scenario = BuildScenario::prepare()?;
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

/// The capability check gates the build targets, so a failing check must stop
/// them before Cargo runs. Asserting on the recorded invocations proves that
/// directly, where relying on Cargo's absence from the sandbox would pass even
/// if the recipe did invoke it.
#[rstest]
#[case("dev-build")]
#[case("dev-test")]
fn a_failed_gate_invokes_cargo_not_at_all(#[case] target: &str) -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    // A usable cargo is present and recording; only mold is missing.
    let cargo = RecordingCargo::install(&sandbox)?;

    let invocation = MakeInvocation::new(target).variable("CARGO", cargo.executable());
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
#[case("dev-build")]
#[case("dev-test")]
fn a_drifting_mold_invokes_cargo_not_at_all(#[case] target: &str) -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    sandbox.write_mold(&sandbox.prefix().join("bin"), "99.0.0")?;
    let cargo = RecordingCargo::install(&sandbox)?;

    let invocation = MakeInvocation::new(target).variable("CARGO", cargo.executable());
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

/// Ask Cargo what the fragment means, rather than only what it contains.
///
/// Parsing the TOML proves the file is well-formed; it cannot prove Cargo
/// accepts the key paths. A backend nested under the wrong table, or a
/// misspelled key, parses perfectly and is then silently ignored. `cargo config
/// get` reports Cargo's own resolved view, so a misplaced key shows up as a
/// missing value instead of passing unnoticed.
///
/// This exercises configuration resolution, not code generation: proving
/// Cranelift and `mold` are genuinely used needs both installed, which
/// `make dev-fast-check` gates at the point of use.
/// The Linux entry is queried through its parent table: `config get` cannot
/// address a key whose name is a quoted `cfg` expression.
#[rstest]
#[case::dev_profile_backend(
    "profile.dev.codegen-backend",
    "profile.dev.codegen-backend = \"cranelift\""
)]
#[case::unstable_flag("unstable.codegen-backend", "unstable.codegen-backend = true")]
#[case::linux_rustflags(
    "target",
    concat!(
        "target.'cfg(target_os = \"linux\")'.rustflags = ",
        "[\"-Zpolonius=next\", \"-Clink-arg=-fuse-ld=mold\"]",
    )
)]
fn cargo_resolves_the_fragment_to_the_intended_settings(
    #[case] query: &str,
    #[case] expected: &str,
) -> Result<()> {
    let output = Command::new(env!("CARGO"))
        .args(["--config", DEV_FAST_CONFIG_PATH, "-Zunstable-options"])
        .args(["config", "get", query])
        .output()
        .with_context(|| format!("ask cargo for {query}"))?;
    let reported = String::from_utf8_lossy(&output.stdout);

    ensure!(
        output.status.success(),
        "cargo should resolve `{query}`, got `{}`",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        reported.contains(expected),
        "cargo should report `{expected}`, got `{reported}`"
    );
    Ok(())
}

/// The recipes pass the fragment by path, so a test asserting only that the
/// path appears would still pass if the file lost its contents. Read it.
#[test]
fn the_cargo_fragment_selects_cranelift_and_mold() -> Result<()> {
    let fragment: toml::Value = toml::from_str(&dev_fast_config()?)?;

    ensure!(
        fragment
            .get("unstable")
            .and_then(|table| table.get("codegen-backend"))
            == Some(&toml::Value::Boolean(true)),
        "the fragment must enable the codegen-backend unstable flag, got `{fragment}`"
    );
    ensure!(
        fragment
            .get("profile")
            .and_then(|table| table.get("dev"))
            .and_then(|table| table.get("codegen-backend"))
            .and_then(toml::Value::as_str)
            == Some("cranelift"),
        "the dev profile must select Cranelift, got `{fragment}`"
    );
    // Release artefacts must never inherit the backend, so no other profile may
    // name one.
    let profiles = fragment
        .get("profile")
        .and_then(toml::Value::as_table)
        .context("the fragment must configure a profile")?;
    for (name, table) in profiles {
        ensure!(
            name == "dev" || table.get("codegen-backend").is_none(),
            "only the dev profile may select a backend, but `{name}` does"
        );
    }

    let linux = fragment
        .get("target")
        .and_then(|table| table.get(r#"cfg(target_os = "linux")"#))
        .and_then(|table| table.get("rustflags"))
        .and_then(toml::Value::as_array)
        .context("the fragment must gate rustflags behind the Linux cfg")?;
    ensure!(
        linux
            .iter()
            .filter_map(toml::Value::as_str)
            .any(|flag| flag.contains("-fuse-ld=mold")),
        "the Linux target must select mold, got `{linux:?}`"
    );
    // Cargo picks one rustflags source rather than merging, and this table
    // outranks `.cargo/config.toml`'s `[build]` table. Dropping the Polonius
    // flag here does not merely diverge from the gate: the tree does not
    // borrow-check without it, so `make dev-build` stops compiling.
    ensure!(
        linux
            .iter()
            .filter_map(toml::Value::as_str)
            .any(|flag| flag == "-Zpolonius=next"),
        "the Linux target must restate the Polonius flag it shadows, got `{linux:?}`"
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
