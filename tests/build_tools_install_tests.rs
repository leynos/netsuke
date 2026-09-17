//! Behavioural tests for the build-tools installer and benchmark scripts.
//!
//! The installer's security-relevant behaviour is that it refuses to unpack an
//! artefact it cannot verify, so these tests serve a locally built tarball over
//! a `file://` URL and vary only the recorded checksum. No network is used.
//!
//! `make install-build-tools` is covered here too, because it is the installer's
//! own entry point; the build and gate recipes live in
//! `build_tools_make_target_tests.rs`.

#![cfg(all(unix, target_os = "linux"))]

use anyhow::{Result, ensure};
use camino::Utf8PathBuf;
use proptest::prelude::*;
use proptest::proptest;
use rstest::rstest;
use test_support::build_tools::{
    FakeRelease, InstallerFixture, InstallerScenario, MakeInvocation, PinOverrides, Sandbox,
    TEST_MOLD_VERSION, WRONG_SHA256, combined, pinned_mold_version, pinned_toolchain,
};

/// Inputs whose checksum file fails verification in the given way.
///
/// Stays with this suite rather than moving to `test_support`: it encodes one
/// suite's failure taxonomy, which is not shared ground.
fn with_failure(
    scenario: &InstallerScenario,
    failure: ChecksumFailure,
) -> Result<InstallerFixture> {
    scenario.fixture(failure.write_checksums(scenario.sandbox(), scenario.release())?)
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
        .sandbox()
        .script("install-build-tools.sh", &fixture.script_env())?;
    let text = combined(&output);

    ensure!(
        output.status.success(),
        "install should succeed, got `{text}`"
    );
    ensure!(
        text.contains(&format!("verified {}", scenario.release().name())),
        "should report the verification, got `{text}`"
    );
    ensure!(
        scenario.installed_mold().as_std_path().is_file(),
        "the tarball root should be stripped so bin/mold lands in the prefix"
    );
    // Assert on the commands rustup actually received, not on the installer's
    // own narration of them: a diagnostic can be emitted without the command
    // ever running.
    let rustup = scenario.sandbox().rustup_invocations()?;
    let toolchain = pinned_toolchain()?;
    ensure!(
        rustup
            .iter()
            .any(|call| call == &format!("toolchain install {toolchain} --profile minimal")),
        "should install the pinned toolchain, recorded `{rustup:?}`"
    );
    // No component is added. The standard names no codegen backend, so an
    // installer that started fetching one would be provisioning something no
    // build asks for.
    ensure!(
        !rustup.iter().any(|call| call.starts_with("component add")),
        "the installer should add no rustup component, recorded `{rustup:?}`"
    );
    Ok(())
}

/// The download must be bounded at both ends.
///
/// A server that completes the handshake and then stops sending leaves an
/// unbounded `curl` waiting forever, so `install-build-tools` hangs and its own
/// failure path is never reached. Asserting on the flags `curl` actually
/// received is the only way to see that from outside: a bounded and an
/// unbounded download look identical unless one is left to stall.
#[rstest]
#[case::connect_timeout("--connect-timeout")]
#[case::stall_floor("--speed-limit")]
#[case::stall_window("--speed-time")]
fn the_download_is_bounded_at_both_ends(#[case] flag: &str) -> Result<()> {
    let scenario = InstallerScenario::prepare()?;
    let fixture = scenario.with_matching_checksum()?;
    let log = scenario.sandbox().home().join("curl-args.log");
    // Record and fail: the arguments are the subject, and refusing the download
    // keeps the case hermetic.
    scenario.sandbox().write_fake(
        &scenario.sandbox().bin(),
        "curl",
        &format!("printf '%s\\n' \"$*\" >> '{log}'\nexit 7"),
    )?;

    let output = scenario
        .sandbox()
        .script("install-build-tools.sh", &fixture.script_env())?;
    ensure!(
        !output.status.success(),
        "a refused download should abort, got `{}`",
        combined(&output)
    );

    let recorded = scenario.sandbox().read_file(&log)?;
    ensure!(
        recorded.contains(flag),
        "curl should receive `{flag}`, got `{recorded}`"
    );
    Ok(())
}

/// A refused artefact must abort before the toolchain half runs, so rustup sees
/// nothing beyond whatever the capability probe needed.
#[test]
fn a_refused_artefact_never_reaches_the_toolchain_install() -> Result<()> {
    let scenario = InstallerScenario::prepare()?;
    let fixture = with_failure(&scenario, ChecksumFailure::Mismatch)?;

    let output = scenario
        .sandbox()
        .script("install-build-tools.sh", &fixture.script_env())?;
    ensure!(!output.status.success(), "install should abort");

    let rustup = scenario.sandbox().rustup_invocations()?;
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
    let fixture = with_failure(&scenario, failure)?;

    let output = scenario
        .sandbox()
        .script("install-build-tools.sh", &fixture.script_env())?;
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
    sandbox.write_rustup(&pinned_toolchain()?)?;
    let committed_version = pinned_mold_version()?;
    let release = FakeRelease::publish(&sandbox, &committed_version)?;

    let output = sandbox.script_with(
        "install-build-tools.sh",
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
        "install-build-tools.sh",
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

proptest! {
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
fn benchmark_emits_a_markdown_table_for_every_variant() -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_mold(&sandbox.prefix().join("bin"), &pinned_mold_version()?)?;
    sandbox.write_rustup(&pinned_toolchain()?)?;
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
        .filter(|line| line.starts_with("| Default") || line.starts_with("| `mold`"))
        .collect();
    ensure!(
        rows.len() == 3,
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

#[test]
fn install_target_forwards_the_prefix_pins_and_release_url() -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_rustup(&pinned_toolchain()?)?;
    let release = FakeRelease::publish(&sandbox, TEST_MOLD_VERSION)?;
    let version_pin = release.write_version_pin(&sandbox)?;
    let checksums = release.write_checksums(&sandbox, release.sha256())?;

    // Pins go through as command-line variables, outranking the Makefile's `?=`
    // defaults; the release URL is read straight from the environment by the
    // script, which is the only channel available for it.
    let invocation = MakeInvocation::new("install-build-tools")
        .variable("MOLD_VERSION_FILE", &version_pin)
        .variable("MOLD_SHA256SUMS_FILE", &checksums)
        .environment("MOLD_RELEASE_BASE_URL", release.base_url());
    let output = sandbox.run_make(&invocation)?;
    let text = combined(&output);

    ensure!(
        output.status.success(),
        "make install-build-tools should succeed, got `{text}`"
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
        sandbox.prefix().join("bin/mold").as_std_path().is_file(),
        "the pinned linker should land in the forwarded prefix"
    );
    Ok(())
}
