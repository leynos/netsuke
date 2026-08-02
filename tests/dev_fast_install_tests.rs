//! Behavioural tests for the `dev-fast` installer and benchmark scripts.
//!
//! The installer's security-relevant behaviour is that it refuses to unpack an
//! artefact it cannot verify, so these tests serve a locally built tarball over
//! a `file://` URL and vary only the recorded checksum. No network is used.
//!
//! The Make targets that wrap these scripts are covered separately in
//! `dev_fast_make_target_tests.rs`.

#![cfg(all(unix, target_os = "linux"))]

use anyhow::{Result, ensure};
use camino::Utf8PathBuf;
use proptest::prelude::*;
use proptest::proptest;
use rstest::rstest;
use test_support::dev_fast::{
    FakeRelease, PinOverrides, Sandbox, combined, pinned_mold_version, pinned_toolchain,
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
    // Assert on the commands rustup actually received, not on the installer's
    // own narration of them: a diagnostic can be emitted without the command
    // ever running.
    let rustup = scenario.sandbox.rustup_invocations()?;
    let toolchain = pinned_toolchain()?;
    ensure!(
        rustup
            .iter()
            .any(|call| call == &format!("toolchain install {toolchain} --profile minimal")),
        "should install the pinned toolchain, recorded `{rustup:?}`"
    );
    ensure!(
        rustup.iter().any(|call| {
            call == &format!(
                "component add rustc-codegen-cranelift-preview --toolchain {toolchain}"
            )
        }),
        "should add the Cranelift component to the pinned toolchain, recorded `{rustup:?}`"
    );
    Ok(())
}

/// A refused artefact must abort before the toolchain half runs, so rustup sees
/// nothing beyond whatever the capability probe needed.
#[test]
fn a_refused_artefact_never_reaches_the_toolchain_install() -> Result<()> {
    let scenario = InstallerScenario::prepare()?;
    let fixture = scenario.with_failure(ChecksumFailure::Mismatch)?;

    let output = scenario
        .sandbox
        .script("install-dev-fast.sh", &fixture.script_env())?;
    ensure!(!output.status.success(), "install should abort");

    let rustup = scenario.sandbox.rustup_invocations()?;
    ensure!(
        !rustup
            .iter()
            .any(|call| call.starts_with("toolchain install") || call.starts_with("component add")),
        "no toolchain should be installed after a refusal, recorded `{rustup:?}`"
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

/// Run without the pin-file variables, the installer must resolve the committed
/// pins from the script's own location.
///
/// It is pointed at a locally published artefact named for the *committed*
/// version, whose digest cannot match the committed checksum. The refusal
/// therefore proves both defaults were read — the version pin supplied the
/// artefact name, and the checksum file supplied the digest it was measured
/// against — while keeping the case hermetic.
#[test]
fn falls_back_to_the_committed_pins_when_no_overrides_are_given() -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    let committed_version = pinned_mold_version()?;
    let release = FakeRelease::publish(&sandbox, &committed_version)?;

    let output = sandbox.script_with(
        "install-dev-fast.sh",
        PinOverrides::Omitted,
        &[("MOLD_RELEASE_BASE_URL", release.base_url())],
    )?;
    let text = combined(&output);

    ensure!(
        !output.status.success(),
        "a stand-in artefact cannot match the committed digest, got `{text}`"
    );
    ensure!(
        text.contains(&format!("mold-{committed_version}-")),
        "the committed version pin should name the artefact, got `{text}`"
    );
    ensure!(
        text.contains("checksum mismatch"),
        "the committed checksum file should reject it, got `{text}`"
    );
    ensure!(
        !sandbox.prefix().join("bin/mold").as_std_path().exists(),
        "nothing should be unpacked when verification fails"
    );
    Ok(())
}

/// An unreadable pin must abort rather than reaching the download as an empty
/// string. `fail` exits, but from inside a command substitution that exit ends
/// only the subshell, so the status has to be propagated explicitly.
#[test]
fn an_unreadable_pin_aborts_before_any_download() -> Result<()> {
    let sandbox = Sandbox::new()?;
    let missing = sandbox.home().join("absent/MOLD_VERSION");

    let output = sandbox.script_with(
        "install-dev-fast.sh",
        PinOverrides::Omitted,
        &[
            ("MOLD_VERSION_FILE", missing.to_string()),
            // A URL that would fail loudly if the installer ever reached it.
            ("MOLD_RELEASE_BASE_URL", "file:///nonexistent".to_owned()),
        ],
    )?;
    let text = combined(&output);

    ensure!(!output.status.success(), "should abort, got `{text}`");
    ensure!(
        text.contains("missing version pin"),
        "should name the unreadable pin, got `{text}`"
    );
    ensure!(
        !text.contains("downloading"),
        "must not attempt a download with an empty version, got `{text}`"
    );
    Ok(())
}

/// A cell holding a one-decimal duration, as `bench-build` formats them.
/// Timings are inherently unstable, so tests assert on shape, not value.
fn is_timing(cell: &str) -> bool {
    // Exactly one point, with digits either side. A looser check accepts `.`,
    // `..`, and `1.2.3`, none of which the benchmark can emit.
    let Some((whole, fraction)) = cell.split_once('.') else {
        return false;
    };
    !whole.is_empty()
        && !fraction.is_empty()
        && whole.chars().all(|c| c.is_ascii_digit())
        && fraction.chars().all(|c| c.is_ascii_digit())
}

/// How one recorded digest relates to the artefact's real one.
#[derive(Copy, Clone, Debug)]
enum Digest {
    /// The artefact's own digest, verbatim.
    Real,
    /// The same digest upper-cased. `sha256sum` compares hex case-insensitively,
    /// so this still verifies — established by probing the tool rather than
    /// assumed.
    RealUpperCased,
    /// A well-formed digest belonging to nothing.
    Wrong,
    /// The real digest with its tail removed.
    Truncated,
}

/// Which artefact a checksum row names.
#[derive(Copy, Clone, Debug)]
enum Named {
    ThisArtefact,
    Other,
}

/// One row of a checksum file.
#[derive(Copy, Clone, Debug)]
struct Row {
    digest: Digest,
    named: Named,
    /// Surrounding whitespace, which the installer's `awk` field split ignores.
    padded: bool,
}

impl Row {
    fn render(self, real: &str, artefact: &str) -> String {
        let digest = match self.digest {
            Digest::Real => real.to_owned(),
            Digest::RealUpperCased => real.to_uppercase(),
            Digest::Wrong => WRONG_SHA256.to_owned(),
            // `get` rather than a bare range: the digest is ASCII hex, but a
            // panicking slice has no place in a model oracle.
            Digest::Truncated => real
                .get(..real.len().saturating_sub(4))
                .unwrap_or_default()
                .to_owned(),
        };
        let name = match self.named {
            Named::ThisArtefact => artefact,
            Named::Other => "mold-0.0.0-x86_64-linux.tar.gz",
        };
        if self.padded {
            format!("   {digest}   {name}   \n")
        } else {
            format!("{digest}  {name}\n")
        }
    }
}

/// The installer must unpack exactly when the file records this artefact
/// exactly once, under the artefact's real digest.
///
/// This is a model, not a lookup table: it predicts the outcome for row lists
/// nobody enumerated. Requiring a single row is deliberate — several rows for
/// one artefact are ambiguous, and the installer refuses rather than picking
/// one. Case is not significant, because `sha256sum` compares hex
/// case-insensitively.
fn model_installs(rows: &[Row]) -> bool {
    let mut selected = rows
        .iter()
        .filter(|row| matches!(row.named, Named::ThisArtefact));
    let Some(only) = selected.next() else {
        return false;
    };
    selected.next().is_none() && matches!(only.digest, Digest::Real | Digest::RealUpperCased)
}

fn digest_strategy() -> impl Strategy<Value = Digest> {
    prop_oneof![
        Just(Digest::Real),
        Just(Digest::RealUpperCased),
        Just(Digest::Wrong),
        Just(Digest::Truncated),
    ]
}

fn row_strategy() -> impl Strategy<Value = Row> {
    (
        digest_strategy(),
        prop_oneof![Just(Named::ThisArtefact), Just(Named::Other)],
        any::<bool>(),
    )
        .prop_map(|(digest, named, padded)| Row {
            digest,
            named,
            padded,
        })
}

proptest! {
    // Each case runs the installer, so keep the corpus small enough to stay a
    // second or two while still ranging over the row space.
    #![proptest_config(ProptestConfig { cases: 24, ..ProptestConfig::default() })]

    /// Whatever rows a checksum file carries, the installer's decision must
    /// match the model, and a refusal must leave nothing unpacked.
    ///
    /// Random digests alone would be uninformative — they never match — so the
    /// strategy ranges over the structural relationships instead: right digest,
    /// wrong digest, truncated, re-cased, another artefact's, duplicated, and
    /// whitespace-padded.
    #[test]
    fn the_installer_verifies_exactly_when_the_model_says_it_should(
        rows in prop::collection::vec(row_strategy(), 0..4)
    ) {
        let scenario = InstallerScenario::prepare()
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let real = scenario.release.sha256().to_owned();
        let artefact = scenario.release.name().to_owned();
        let contents: String = rows.iter().map(|row| row.render(&real, &artefact)).collect();

        let checksums = scenario.sandbox.home().join("SHA256SUMS");
        scenario
            .sandbox
            .write_file(&checksums, &contents)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let fixture = scenario
            .fixture(checksums)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;

        let output = scenario
            .sandbox
            .script("install-dev-fast.sh", &fixture.script_env())
            .map_err(|error| TestCaseError::fail(error.to_string()))?;

        let expected = model_installs(&rows);
        prop_assert_eq!(
            output.status.success(),
            expected,
            "rows {:?} should {} install, got `{}`",
            rows,
            if expected { "" } else { "not" },
            combined(&output)
        );
        prop_assert_eq!(
            scenario.installed_mold().as_std_path().exists(),
            expected,
            "unpacked state should follow the verification decision for {:?}",
            rows
        );
    }

    /// A timing cell is exactly `<digits>.<digits>`. Generating around that
    /// shape covers the malformed neighbours — bare dots, multiple points, a
    /// missing side — that a hand-picked example list tends to miss.
    #[test]
    fn is_timing_accepts_exactly_one_point_between_digits(
        cell in r"[0-9.]{0,6}"
    ) {
        let expected = {
            let mut parts = cell.split('.');
            let whole = parts.next().unwrap_or_default();
            let fraction = parts.next().unwrap_or_default();
            parts.next().is_none()
                && cell.contains('.')
                && !whole.is_empty()
                && !fraction.is_empty()
        };
        prop_assert_eq!(is_timing(&cell), expected, "cell `{}`", cell);
    }

    /// Whatever the digits, a well-formed one-decimal timing is accepted.
    #[test]
    fn is_timing_accepts_any_one_decimal_duration(whole in 0u32..100_000, fraction in 0u32..10) {
        let cell = format!("{whole}.{fraction}");
        prop_assert!(is_timing(&cell), "cell `{}`", cell);
    }
}

#[test]
fn benchmark_emits_a_markdown_table_for_both_paths() -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_mold(&sandbox.prefix().join("bin"), &pinned_mold_version()?)?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    let cargo = sandbox.write_fake(&sandbox.bin(), "cargo", "exit 0")?;
    let touch_file = sandbox.home().join("bench-touch");
    sandbox.write_file(&touch_file, "")?;

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
