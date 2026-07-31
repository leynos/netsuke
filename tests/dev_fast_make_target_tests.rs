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

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use rstest::rstest;
use std::fs::File;
use std::time::{Duration, UNIX_EPOCH};
use test_support::dev_fast::{
    CargoInvocation, FakeRelease, MakeInvocation, RecordingCargo, Sandbox, TargetState, combined,
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
    BuildTarget {
        name: "dev-test",
        // Mirrors `make test-nextest`, so the accelerated loop and the gate run
        // the same runner under the same `.config/nextest.toml`.
        subcommand: &["nextest", "run", "--all-targets", "--all-features"],
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

/// Timestamp stamped on the benchmark's touch file before the run, chosen far
/// enough in the past that any `touch` is unambiguously newer. Comparing
/// against a fixed baseline keeps the assertion deterministic, where comparing
/// the two passes to each other would depend on filesystem timestamp
/// granularity.
const BASELINE_MTIME: i64 = 1_600_000_000;

/// Target-directory slugs the benchmark uses, one per variant.
const DEFAULT_SLUG: &str = "default";
const DEV_FAST_SLUG: &str = "dev-fast";

/// Create the touch file with [`BASELINE_MTIME`], returning that timestamp.
fn write_with_old_mtime(path: &Utf8Path) -> Result<i64> {
    let file = File::create(path.as_std_path())
        .with_context(|| format!("create bench touch target {path}"))?;
    let baseline = UNIX_EPOCH + Duration::from_secs(BASELINE_MTIME.unsigned_abs());
    file.set_modified(baseline)
        .with_context(|| format!("backdate {path}"))?;
    Ok(BASELINE_MTIME)
}

#[test]
fn bench_target_emits_both_variant_rows() -> Result<()> {
    let scenario = BuildScenario::prepare()?;
    let sandbox = &scenario.sandbox;
    // Touch a disposable file rather than `src/main.rs`, so the benchmark does
    // not invalidate the working tree's build cache.
    let touch_file = sandbox.home().join("bench-touch");
    let baseline_mtime = write_with_old_mtime(&touch_file)?;

    // Seed both target directories, modelling a re-run. Without this the
    // benchmark's `rm -rf` would be indistinguishable from doing nothing,
    // because a fresh sandbox has no artefacts to remove.
    let bench_root = sandbox.home().join("bench");
    for slug in [DEFAULT_SLUG, DEV_FAST_SLUG] {
        std::fs::create_dir_all(bench_root.join(slug).as_std_path())
            .with_context(|| format!("seed stale {slug} target directory"))?;
    }

    let invocation = MakeInvocation::new("bench-build")
        .variable("CARGO", scenario.cargo.executable())
        .environment("BENCH_ROOT", &bench_root)
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

    // The benchmark measures the default variant first, then the accelerated
    // one, each as a clean pass followed by an incremental pass.
    let mut pairs = invocations.chunks_exact(2);
    let default_pass = BenchVariant::from_pair(DEFAULT_SLUG, pairs.next(), false)?;
    let dev_fast_pass = BenchVariant::from_pair(DEV_FAST_SLUG, pairs.next(), true)?;
    for variant in [&default_pass, &dev_fast_pass] {
        variant.check(&pinned_toolchain()?)?;
    }

    ensure!(
        default_pass.target_dir() != dev_fast_pass.target_dir(),
        "variants must not share a target directory, got `{}` and `{}`",
        default_pass.target_dir(),
        dev_fast_pass.target_dir()
    );

    // Only the very first pass runs before any touch; each variant touches the
    // file between its own two passes, so every later pass must see a newer
    // timestamp. Comparing against a backdated baseline rather than between
    // passes keeps this free of filesystem timestamp granularity.
    let first = invocations.first().context("expected a recorded pass")?;
    ensure!(
        first.touch_mtime() == Some(baseline_mtime),
        "the first clean pass should precede any touch, got {:?}",
        first.touch_mtime()
    );
    for (index, pass) in invocations.iter().enumerate().skip(1) {
        ensure!(
            pass.touch_mtime()
                .is_some_and(|mtime| mtime > baseline_mtime),
            "pass {index} should follow a touch, got {:?}",
            pass.touch_mtime()
        );
    }
    Ok(())
}

/// One variant's pair of recorded builds: the clean pass then the incremental.
struct BenchVariant<'a> {
    label: &'a str,
    clean: &'a CargoInvocation,
    incremental: &'a CargoInvocation,
    /// Whether this variant is the accelerated one, which alone carries the
    /// Cranelift fragment and the pinned toolchain.
    expects_fragment: bool,
}

impl<'a> BenchVariant<'a> {
    /// Build a descriptor from the variant's recorded pair of passes.
    fn from_pair(
        label: &'a str,
        pair: Option<&'a [CargoInvocation]>,
        expects_fragment: bool,
    ) -> Result<Self> {
        let passes = pair.with_context(|| format!("no recorded passes for `{label}`"))?;
        let clean = passes
            .first()
            .with_context(|| format!("no clean pass for `{label}`"))?;
        let incremental = passes
            .get(1)
            .with_context(|| format!("no incremental pass for `{label}`"))?;
        Ok(Self {
            label,
            clean,
            incremental,
            expects_fragment,
        })
    }

    fn target_dir(&self) -> &str {
        self.clean.target_dir()
    }

    fn check(&self, toolchain: &str) -> Result<()> {
        let label = self.label;
        for pass in [self.clean, self.incremental] {
            ensure!(
                pass.contains_sequence(&["build", "--bin", "netsuke"]),
                "`{label}` should build the binary, got `{:?}`",
                pass.arguments()
            );
            ensure!(
                pass.contains_sequence(&["--config", DEV_FAST_CONFIG]) == self.expects_fragment,
                "`{label}` fragment expectation ({}) not met, got `{:?}`",
                self.expects_fragment,
                pass.arguments()
            );
            let expected_toolchain = if self.expects_fragment { toolchain } else { "" };
            ensure!(
                pass.toolchain() == expected_toolchain,
                "`{label}` should run under `{expected_toolchain}`, got `{}`",
                pass.toolchain()
            );
        }

        ensure!(
            self.clean.target_dir() == self.incremental.target_dir(),
            "`{label}` should reuse one target directory across its passes"
        );
        ensure!(
            self.clean.target_dir().ends_with(label),
            "`{label}` should measure in its own directory, got `{}`",
            self.clean.target_dir()
        );

        // The harness removes the directory before the clean pass, and the
        // clean pass leaves it behind, so this ordering is what distinguishes
        // a genuine clean/incremental pair from two identical builds.
        ensure!(
            self.clean.target_state() == TargetState::Absent,
            "`{label}` clean pass should start from an empty target directory, got {:?}",
            self.clean.target_state()
        );
        ensure!(
            self.incremental.target_state() == TargetState::Present,
            "`{label}` incremental pass should reuse the clean pass's output, got {:?}",
            self.incremental.target_state()
        );

        Ok(())
    }
}
