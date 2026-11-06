//! Boundary tests for the local `kani-check` query.
//!
//! `make kani-check` must stay a local read-only query: it reads the pin and
//! asks the installed verifier for its version. These tests poison the remote
//! prover-tools runner so any attempt to resolve it fails loudly, proving the
//! query path cannot reach the network or write the uv cache. The isolation
//! case goes further and removes the ability to write or reach the network at
//! all, then checks that the query still answers.

#![cfg(unix)]

#[path = "prover_tools_kani_check_tests/support.rs"]
mod support;

use anyhow::{Context, Result, ensure};
#[cfg(target_os = "linux")]
use std::path::Path;
use support::Fixture;

/// Check that a matching version succeeds and that the remote runner is never
/// consulted.
#[test]
fn kani_check_reads_the_installed_binary_without_remote_resolution() -> Result<()> {
    let fixture = Fixture::new()?;
    let (output, marker) = fixture.run_fake_kani("#!/bin/sh\nprintf 'cargo-kani 0.67.0\\n'\n")?;

    ensure!(
        output.status.success(),
        "make kani-check should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        stdout.contains("Kani 0.67.0 matches"),
        "the query should report the matching version, got `{stdout}`"
    );
    fixture.ensure_remote_runner_unused(&marker)
}

/// Check that a version mismatch fails and names the expected and actual
/// versions.
#[test]
fn kani_check_reports_a_version_mismatch() -> Result<()> {
    let fixture = Fixture::new()?;
    let (stderr, marker) =
        fixture.run_failing_check("#!/bin/sh\nprintf 'cargo-kani 0.66.0\\n'\n")?;

    ensure!(
        stderr.contains("expected Kani 0.67.0") && stderr.contains("found 0.66.0"),
        "the mismatch diagnostic should name the expected and actual versions, got `{stderr}`"
    );
    fixture.ensure_remote_runner_unused(&marker)
}

/// Check that a failing verifier propagates its status through the recipe
/// diagnostic.
#[test]
fn kani_check_preserves_the_verifier_exit_status() -> Result<()> {
    let fixture = Fixture::new()?;
    let (stderr, marker) = fixture.run_failing_check("#!/bin/sh\nexit 42\n")?;

    ensure!(
        stderr.contains("prover-tools: target=kani-check failed exit=42"),
        "the diagnostic should preserve the verifier status, got `{stderr}`"
    );
    fixture.ensure_remote_runner_unused(&marker)
}

/// Check that version output without a semantic version is rejected rather than
/// compared loosely.
#[test]
fn kani_check_rejects_unparseable_version_output() -> Result<()> {
    let fixture = Fixture::new()?;
    let (stderr, marker) = fixture.run_failing_check("#!/bin/sh\nprintf 'unknown build\\n'\n")?;

    ensure!(
        stderr.contains("could not parse Kani version"),
        "the diagnostic should report the unparseable output, got `{stderr}`"
    );
    fixture.ensure_remote_runner_unused(&marker)
}

/// Check that the default active-home command survives an apostrophe in
/// `CARGO_HOME`.
#[test]
fn kani_check_handles_apostrophes_in_the_active_cargo_home() -> Result<()> {
    let fixture = Fixture::new()?;
    let cargo_home = fixture.path("cargo's home with spaces");
    fixture.create_dir_all("cargo's home with spaces/bin")?;
    fixture.write_executable(
        "cargo's home with spaces/bin/cargo-kani",
        "#!/bin/sh\nprintf 'cargo-kani 0.67.0\\n'\n",
    )?;
    let version_file = fixture.write_version_file()?;
    let (mut command, marker) = fixture.make_check_command()?;
    let output = command
        .arg(format!("KANI_VERSION_FILE={}", version_file.display()))
        .arg(format!("CARGO_HOME={}", cargo_home.display()))
        .output()
        .context("run make kani-check with an apostrophe in CARGO_HOME")?;

    ensure!(
        output.status.success(),
        "make kani-check should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    fixture.ensure_remote_runner_unused(&marker)
}

/// Check that the recipe asks the verifier for its version and passes no other
/// argument.
#[test]
fn kani_check_invokes_the_verifier_with_the_version_flag() -> Result<()> {
    let fixture = Fixture::new()?;
    let kani = fixture.fake_kani(&format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" > '{}'\nprintf 'cargo-kani 0.67.0\\n'\n",
        fixture.path("kani-argv").display()
    ))?;
    let version_file = fixture.write_version_file()?;
    let (output, marker) = fixture.run_check(&kani, &version_file)?;

    ensure!(
        output.status.success(),
        "make kani-check should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        fixture.read_to_string("kani-argv")?.trim() == "--version",
        "the recipe should ask for the version and nothing else, got `{}`",
        fixture.read_to_string("kani-argv")?.trim()
    );
    fixture.ensure_remote_runner_unused(&marker)
}

/// Check that the expected version comes from the version file rather than
/// from a copy carried inside the recipe.
#[test]
fn kani_check_reads_the_expected_version_from_the_file() -> Result<()> {
    let fixture = Fixture::new()?;
    let kani = fixture.fake_kani("#!/bin/sh\nprintf 'cargo-kani 0.68.0\\n'\n")?;
    let version_file = fixture.write_version_file_pinning("0.68.0")?;
    let (output, marker) = fixture.run_check(&kani, &version_file)?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    ensure!(
        output.status.success(),
        "the query should accept a verifier matching a re-pinned file: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        stdout.contains("Kani 0.68.0 matches"),
        "the query should compare against the file's pin, got `{stdout}`"
    );
    fixture.ensure_remote_runner_unused(&marker)
}

/// Check that an unreadable version file fails with a diagnostic naming the
/// read, rather than being treated as an empty pin.
///
/// The file is replaced with a directory: `cat` refuses one for every user,
/// including a root runner, so the case does not depend on the test's uid.
#[test]
fn kani_check_reports_an_unreadable_version_file() -> Result<()> {
    let fixture = Fixture::new()?;
    let kani = fixture.fake_kani("#!/bin/sh\nprintf 'cargo-kani 0.67.0\\n'\n")?;
    fixture.create_dir_all("KANI_VERSION")?;
    let (output, marker) = fixture.run_check(&kani, &fixture.path("KANI_VERSION"))?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    ensure!(
        !output.status.success(),
        "an unreadable version file must fail the query, got `{stderr}`"
    );
    ensure!(
        stderr.contains("could not read"),
        "the diagnostic should name the unreadable pin, got `{stderr}`"
    );
    fixture.ensure_remote_runner_unused(&marker)
}

/// Check that the query succeeds with network access and filesystem writes
/// blocked, and that the sandbox actually applied both restrictions.
///
/// This is the boundary the finding asks for: the recipe is read-only by
/// construction, so the test removes the ability to write or reach the network
/// and shows the query still answers. The reported restriction lines prove the
/// namespace took effect, which keeps the case from passing vacuously on a host
/// where `unshare` silently did nothing. The `control` line is the converse
/// check: it names a path that really is writable, so a probe that reported
/// every path as blocked because redirection itself was broken would fail here
/// rather than pass.
///
/// # Errors
///
/// Returns an error if the fixture cannot be built, if the isolated query
/// fails, or if the sandbox reports that a restriction did not apply.
#[cfg(target_os = "linux")]
#[test]
fn kani_check_runs_without_network_or_writable_storage() -> Result<()> {
    let fixture = Fixture::new()?;
    if let Some(reason) = support::sandbox::unavailable() {
        return support::sandbox::skip(&reason);
    }
    let (output, marker) = fixture.run_sandboxed_check(Path::new(env!("CARGO_MANIFEST_DIR")))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    ensure!(
        output.status.success(),
        "the isolated query should succeed, got stdout `{stdout}` and stderr `{stderr}`"
    );
    for expected in [
        "control=writable",
        "repo=read-only",
        "fixture=read-only",
        "rootdir=read-only",
        "usr=read-only",
        "etc=read-only",
        "tmp=read-only",
        "ifaces=lo,",
        "dns=blocked",
        "Kani 0.67.0 matches",
    ] {
        ensure!(
            stdout.contains(expected),
            "the isolated query should report `{expected}`, got `{stdout}`"
        );
    }
    // `HOME` is inherited from the caller, so the chroot contains it only when
    // the caller has one. Either way it must not be writable, and an absent
    // path cannot be written.
    ensure!(
        stdout
            .lines()
            .any(|line| line == "home=read-only" || line == "home=absent"),
        "the isolated query should report `home` as read-only or absent, got `{stdout}`"
    );
    // The control path is the only one allowed to be writable; anything else
    // reporting `=writable` means the sandbox left a hole.
    for line in stdout.lines().filter(|line| line.contains('=')) {
        ensure!(
            line == "control=writable" || !line.ends_with("=writable"),
            "only the control path may be writable, got `{line}` in `{stdout}`"
        );
    }
    ensure!(
        !stderr.contains("prover-tools runner invoked"),
        "the isolated query must not resolve the remote runner, got `{stderr}`"
    );
    fixture.ensure_remote_runner_unused(&marker)
}
