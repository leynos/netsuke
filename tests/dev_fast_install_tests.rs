//! Behavioural tests for the `dev-fast` installer and benchmark scripts.
//!
//! The installer's security-relevant behaviour is that it refuses to unpack an
//! artefact it cannot verify, so these tests serve a locally built tarball over
//! a `file://` URL and vary only the recorded checksum. No network is used.
//!
//! The Make targets that wrap these scripts are covered separately in
//! `dev_fast_make_target_tests.rs`.

#![cfg(all(unix, target_os = "linux"))]

use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use rstest::rstest;
use std::fs;
use test_support::dev_fast::{
    FakeRelease, Sandbox, combined, pinned_mold_version, pinned_toolchain,
};

/// The version the fake release is published as. Deliberately not a real mold
/// version, so a test that accidentally reached the network would fail rather
/// than silently succeed against an upstream artefact.
const TEST_MOLD_VERSION: &str = "9.9.9";

/// A digest that cannot match any artefact, for the mismatch case.
const WRONG_SHA256: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// The installer inputs pointing at a local fake release.
///
/// Grouping them keeps the version, pin path, checksum path, and URL travelling
/// together as one value instead of as a handful of interchangeable strings.
struct InstallerFixture {
    version_pin: Utf8PathBuf,
    checksums: Utf8PathBuf,
    base_url: String,
}

impl InstallerFixture {
    /// Environment overrides for a direct `install-dev-fast.sh` invocation.
    fn script_env(&self) -> Vec<(&'static str, String)> {
        vec![
            ("MOLD_VERSION_FILE", self.version_pin.to_string()),
            ("MOLD_SHA256SUMS_FILE", self.checksums.to_string()),
            ("MOLD_RELEASE_BASE_URL", self.base_url.clone()),
        ]
    }
}

/// A sandbox with a published fake release and a usable `rustup`.
struct InstallerScenario {
    sandbox: Sandbox,
    release: FakeRelease,
}

impl InstallerScenario {
    /// Publish a release and make the Cranelift toolchain appear installed, so
    /// only the linker half of the installer is under test.
    fn prepare() -> Result<Self> {
        let sandbox = Sandbox::new()?;
        sandbox.write_rustup(&pinned_toolchain()?, true)?;
        let release = FakeRelease::publish(&sandbox, TEST_MOLD_VERSION)?;
        Ok(Self { sandbox, release })
    }

    /// Fixture recording the release's real digest, so verification passes.
    fn with_matching_checksum(&self) -> Result<InstallerFixture> {
        self.fixture(
            self.release
                .write_checksums(&self.sandbox, self.release.sha256())?,
        )
    }

    /// Fixture whose checksum file fails verification in the given way.
    fn with_failure(&self, failure: ChecksumFailure) -> Result<InstallerFixture> {
        self.fixture(failure.write_checksums(&self.sandbox, &self.release)?)
    }

    fn fixture(&self, checksums: Utf8PathBuf) -> Result<InstallerFixture> {
        Ok(InstallerFixture {
            version_pin: self.release.write_version_pin(&self.sandbox)?,
            checksums,
            base_url: self.release.base_url(),
        })
    }

    fn installed_mold(&self) -> Utf8PathBuf {
        self.sandbox.prefix().join("bin/mold")
    }
}

/// The ways verification can legitimately fail.
///
/// A closed enum rather than a pair of strings: each variant owns both the
/// checksum file it needs and the diagnostic the installer must emit, so the two
/// cannot drift apart.
#[derive(Copy, Clone, Debug)]
enum ChecksumFailure {
    /// The artefact is listed, but under a different digest.
    Mismatch,
    /// The checksum file is well-formed but says nothing about this artefact.
    MissingEntry,
}

impl ChecksumFailure {
    fn write_checksums(self, sandbox: &Sandbox, release: &FakeRelease) -> Result<Utf8PathBuf> {
        match self {
            Self::Mismatch => release.write_checksums(sandbox, WRONG_SHA256),
            Self::MissingEntry => release.write_checksums_omitting_this_artefact(sandbox),
        }
    }

    const fn expected_diagnostic(self) -> &'static str {
        match self {
            Self::Mismatch => "checksum mismatch",
            Self::MissingEntry => "no checksum recorded",
        }
    }
}

#[test]
fn installs_and_records_the_verification_when_the_checksum_matches() -> Result<()> {
    let scenario = InstallerScenario::prepare()?;
    let fixture = scenario.with_matching_checksum()?;

    let output = scenario
        .sandbox
        .script("install-dev-fast.sh", &fixture.script_env())?;
    let text = combined(&output);

    ensure!(
        output.status.success(),
        "install should succeed, got `{text}`"
    );
    ensure!(
        text.contains(&format!("verified {}", scenario.release.name())),
        "should report the verification, got `{text}`"
    );
    ensure!(
        scenario.installed_mold().as_std_path().is_file(),
        "the tarball root should be stripped so bin/mold lands in the prefix"
    );
    ensure!(
        text.contains("rustc-codegen-cranelift-preview"),
        "should install the Cranelift component, got `{text}`"
    );
    Ok(())
}

/// Refusing an unverifiable artefact is the point of the checksum file, so both
/// failure modes must abort before anything is unpacked.
#[rstest]
#[case::mismatch(ChecksumFailure::Mismatch)]
#[case::missing_entry(ChecksumFailure::MissingEntry)]
fn refuses_to_install_an_unverifiable_artefact(#[case] failure: ChecksumFailure) -> Result<()> {
    let scenario = InstallerScenario::prepare()?;
    let fixture = scenario.with_failure(failure)?;

    let output = scenario
        .sandbox
        .script("install-dev-fast.sh", &fixture.script_env())?;
    let text = combined(&output);

    ensure!(
        !output.status.success(),
        "install should abort, got `{text}`"
    );
    ensure!(
        text.contains(failure.expected_diagnostic()),
        "should explain the refusal (`{}`), got `{text}`",
        failure.expected_diagnostic()
    );
    ensure!(
        !scenario.installed_mold().as_std_path().exists(),
        "nothing should be unpacked when verification fails"
    );
    Ok(())
}

/// A cell holding a one-decimal duration, as `bench-build` formats them.
/// Timings are inherently unstable, so tests assert on shape, not value.
fn is_timing(cell: &str) -> bool {
    !cell.is_empty() && cell.contains('.') && cell.chars().all(|c| c.is_ascii_digit() || c == '.')
}

#[test]
fn benchmark_emits_a_markdown_table_for_both_paths() -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_mold(&sandbox.prefix().join("bin"), &pinned_mold_version()?)?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    let cargo = sandbox.write_fake(&sandbox.bin(), "cargo", "exit 0")?;
    let touch_file = sandbox.home().join("bench-touch");
    fs::write(touch_file.as_std_path(), "").context("create bench touch target")?;

    let output = sandbox.script(
        "bench-build.sh",
        &[
            ("CARGO", cargo.to_string()),
            ("BENCH_ROOT", sandbox.home().join("bench").to_string()),
            ("BENCH_TOUCH_FILE", touch_file.to_string()),
        ],
    )?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    ensure!(
        output.status.success(),
        "benchmark should succeed, got `{}`",
        combined(&output)
    );
    ensure!(
        stdout.contains("| Variant | Clean build (s) | Incremental build (s) |"),
        "should emit the table header, got `{stdout}`"
    );
    let rows: Vec<&str> = stdout
        .lines()
        .filter(|line| line.starts_with("| Default") || line.starts_with("| dev-fast"))
        .collect();
    ensure!(
        rows.len() == 2,
        "should report one row per variant, got `{stdout}`"
    );
    for row in rows {
        let measurements = row.split('|').map(str::trim).filter(|cell| is_timing(cell));
        ensure!(
            measurements.count() == 2,
            "row should carry two decimal timings, got `{row}`"
        );
    }
    Ok(())
}
