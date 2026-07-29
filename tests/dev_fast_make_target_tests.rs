//! Behavioural tests for the `dev-fast` Make targets' success paths.
//!
//! The scripts are covered directly elsewhere; what these tests pin down is the
//! contract the recipes themselves establish — which toolchain is selected,
//! which configuration fragment is passed, which Cargo subcommand runs, and that
//! the install prefix leads `PATH` so `-fuse-ld=mold` resolves the pinned
//! linker. A fake `cargo` records each invocation so those become checked facts
//! rather than assumptions.
//!
//! Every case is hermetic: no network, and no real mold, rustup, or Cargo.

#![cfg(all(unix, target_os = "linux"))]

use anyhow::{Result, ensure};
use camino::Utf8PathBuf;
use rstest::rstest;
use test_support::dev_fast::{
    CargoInvocation, FakeRelease, MakeInvocation, RecordingCargo, Sandbox, combined,
    pinned_mold_version, pinned_toolchain,
};

/// The version the fake release is published as; see the installer tests.
const TEST_MOLD_VERSION: &str = "9.9.9";

/// The fragment the recipes must hand to Cargo. Hard-coded rather than derived,
/// so a change to the committed path fails a test instead of passing silently.
const DEV_FAST_CONFIG: &str = "tools/dev-fast/config.toml";

/// A sandbox whose prerequisites all pass, with a recording `cargo` installed.
struct BuildScenario {
    sandbox: Sandbox,
    cargo: RecordingCargo,
}

impl BuildScenario {
    fn prepare() -> Result<Self> {
        let sandbox = Sandbox::new()?;
        sandbox.write_mold(&sandbox.prefix().join("bin"), &pinned_mold_version()?)?;
        sandbox.write_rustup(&pinned_toolchain()?, true)?;
        let cargo = RecordingCargo::install(&sandbox)?;
        Ok(Self { sandbox, cargo })
    }

    /// Run `target`, pointing `CARGO` at the recording fake.
    fn run(&self, target: &str) -> Result<CargoInvocation> {
        let invocation = MakeInvocation::new(target).variable("CARGO", self.cargo.executable());
        let output = self.sandbox.run_make(&invocation)?;
        ensure!(
            output.status.success(),
            "make {target} should succeed, got `{}`",
            combined(&output)
        );
        self.cargo.sole_invocation()
    }

    fn prefix_bin(&self) -> Utf8PathBuf {
        self.sandbox.prefix().join("bin")
    }
}

/// What a given build target must ask Cargo to do.
#[derive(Copy, Clone, Debug)]
struct BuildTarget {
    name: &'static str,
    subcommand: &'static [&'static str],
}

#[rstest]
#[case::dev_build(BuildTarget { name: "dev-build", subcommand: &["build", "--bin", "netsuke"] })]
#[case::dev_test(
    BuildTarget { name: "dev-test", subcommand: &["test", "--all-targets", "--all-features"] }
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
        invocation.contains_sequence(&["--config", DEV_FAST_CONFIG]),
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
    // The linker is resolved by PATH order, so leading the prefix is the whole
    // mechanism by which the pinned mold, and not a system one, gets used.
    ensure!(
        invocation.path_starts_with(&scenario.prefix_bin()),
        "`{}` should lead PATH with the install prefix, got `{}`",
        target.name,
        invocation.path()
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

#[test]
fn bench_target_emits_both_variant_rows() -> Result<()> {
    let scenario = BuildScenario::prepare()?;
    let sandbox = &scenario.sandbox;
    // Touch a disposable file rather than `src/main.rs`, so the benchmark does
    // not invalidate the working tree's build cache.
    let touch_file = sandbox.home().join("bench-touch");
    std::fs::write(touch_file.as_std_path(), "").map_err(anyhow::Error::from)?;

    let invocation = MakeInvocation::new("bench-build")
        .variable("CARGO", scenario.cargo.executable())
        .environment("BENCH_ROOT", sandbox.home().join("bench"))
        .environment("BENCH_TOUCH_FILE", &touch_file);
    let output = sandbox.run_make(&invocation)?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    ensure!(
        output.status.success(),
        "make bench-build should succeed, got `{}`",
        combined(&output)
    );
    for variant in [
        "| Default (LLVM, platform linker) |",
        "| dev-fast (Cranelift,",
    ] {
        ensure!(
            stdout.contains(variant),
            "table should carry the `{variant}` row, got `{stdout}`"
        );
    }
    // Four builds: clean and incremental, for each of the two variants.
    let invocations = scenario.cargo.invocations()?;
    ensure!(
        invocations.len() == 4,
        "should measure two builds per variant, recorded {}",
        invocations.len()
    );
    Ok(())
}
