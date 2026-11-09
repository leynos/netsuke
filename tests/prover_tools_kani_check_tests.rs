//! Boundary tests for the local `kani-check` query.
//!
//! `make kani-check` must stay a local read-only query: it reads the pin and
//! asks the installed verifier for its version. These tests poison the remote
//! prover-tools runner so any attempt to resolve it fails loudly, proving the
//! query path cannot reach the network or write the uv cache. The isolation
//! case goes further and removes the ability to write or reach the network at
//! all, then checks that the query still answers.

#![cfg(unix)]

#[path = "support/makefile.rs"]
mod makefile;

#[path = "prover_tools_kani_check_tests/support/mod.rs"]
mod support;

// The static half of the read-only contract: the sandbox below proves a write
// cannot succeed, and this module proves the recipe never attempts one.
#[path = "prover_tools_kani_check_tests/recipe.rs"]
mod recipe;

// The runtime half: the sandbox traces the query, and this module turns the
// trace into the attempt findings the boundary case asserts are empty.
#[path = "prover_tools_kani_check_tests/trace.rs"]
mod trace;

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
        stderr.contains("prover-tools: target=kani-check category=verifier-failed failed exit=42"),
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
        stderr.contains("category=version-unparsable")
            && stderr.contains("could not parse a Kani version from the verifier output"),
        "the diagnostic should categorise the unparseable output, got `{stderr}`"
    );
    ensure!(
        !stderr.contains("unknown build"),
        "the diagnostic must not echo the verifier's own output, got `{stderr}`"
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
/// blocked, that it attempts neither, and that the attempt reader fires.
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
/// A refused attempt and no attempt look identical in an exit status once a
/// caller discards the error, so the queries are traced instead. The real trace
/// must contain no write-intent open, mutation or network connection. The
/// control verifier discards a write and a connection and still reports the
/// pinned version, and its trace must name both — otherwise the reader, not the
/// recipe, is what passed. The trace wraps `make`, so the recipe's own commands
/// are followed too, and the case asserts that by requiring the pin read to
/// appear in the trace.
///
/// When `NETSUKE_KANI_ISOLATION_VERIFIER` names an installed verifier, a third
/// traced query runs that binary through the same recipe and its trace must be
/// equally clean. CI sets it to the pinned `cargo-kani` it installed, so the
/// real executable — not only the stub — is exercised inside the boundary.
///
/// # Errors
///
/// Returns an error if the fixture cannot be built, if the isolated query
/// fails, if the sandbox reports that a restriction did not apply, or if the
/// control trace does not name the attempts it was built to make.
#[cfg(target_os = "linux")]
#[test]
fn kani_check_runs_without_network_or_writable_storage() -> Result<()> {
    let fixture = Fixture::new()?;
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let installed = support::installed_verifier()?;
    if let Some(reason) = support::sandbox::unavailable(repo, fixture.root(), installed.as_ref()) {
        return support::sandbox::skip(&reason);
    }
    if !support::sandbox::tracer_available() {
        return support::sandbox::skip("strace is not installed, so attempts cannot be read back");
    }
    let run = fixture.run_sandboxed_check(repo, installed.as_ref())?;
    let stdout = String::from_utf8_lossy(&run.output.stdout);
    let stderr = String::from_utf8_lossy(&run.output.stderr);

    ensure!(
        run.output.status.success(),
        "the isolated query should succeed, got stdout `{stdout}` and stderr `{stderr}`"
    );
    ensure_restrictions_reported(&stdout)?;
    ensure_home_unwritable(&stdout)?;
    ensure_only_control_writable(&stdout)?;
    ensure!(
        !stderr.contains("prover-tools runner invoked"),
        "the isolated query must not resolve the remote runner, got `{stderr}`"
    );
    ensure!(
        stdout.contains("control-exit=0"),
        "the control verifier must succeed while discarding its failed attempts, \
         got stdout `{stdout}` and stderr `{stderr}`"
    );
    ensure_query_attempts_nothing(&run.query_trace)?;
    ensure_recipe_commands_traced(&run.query_trace)?;
    ensure_control_attempts_traced(&run.control_trace)?;
    ensure_installed_query_works(&run, &stdout, &stderr)?;
    fixture.ensure_remote_runner_unused(&run.marker)
}

/// The reports a sandboxed query must make once the sandbox has applied every
/// restriction: each blocked path, the absence of a usable interface and DNS,
/// and the successful version match.
#[cfg(target_os = "linux")]
const REQUIRED_REPORTS: [&str; 10] = [
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
];

/// Assert the sandbox reported every restriction it applied.
#[cfg(target_os = "linux")]
fn ensure_restrictions_reported(stdout: &str) -> Result<()> {
    for expected in REQUIRED_REPORTS {
        ensure!(
            stdout.contains(expected),
            "the isolated query should report `{expected}`, got `{stdout}`"
        );
    }
    Ok(())
}

/// Assert the inherited home directory was reported unwritable or absent.
///
/// `HOME` comes from the caller, so the chroot contains it only when the caller
/// has one. Either state satisfies the boundary; a writable one does not.
#[cfg(target_os = "linux")]
fn ensure_home_unwritable(stdout: &str) -> Result<()> {
    let reported = stdout
        .lines()
        .any(|line| line == "home=read-only" || line == "home=absent");
    ensure!(
        reported,
        "the isolated query should report `home` as read-only or absent, got `{stdout}`"
    );
    Ok(())
}

/// Assert no path other than the deliberate control is reported writable.
///
/// The control names a path that really is writable, so a sandbox that blocked
/// everything — including its own probe — fails here rather than passing.
#[cfg(target_os = "linux")]
fn ensure_only_control_writable(stdout: &str) -> Result<()> {
    for line in stdout.lines().filter(|line| line.contains('=')) {
        ensure!(
            line == "control=writable" || !line.ends_with("=writable"),
            "only the control path may be writable, got `{line}` in `{stdout}`"
        );
    }
    Ok(())
}

/// Assert the query's own trace names no attempted write or connection.
#[cfg(target_os = "linux")]
fn ensure_query_attempts_nothing(trace: &str) -> Result<()> {
    let findings = trace::attempts(trace);
    ensure!(
        findings.is_empty(),
        "the isolated query must attempt no write or connection, got {findings:?}"
    );
    Ok(())
}

/// Assert the trace covers the recipe's own commands, not only the verifier.
///
/// The recipe reads the pin file before it runs anything, and the trace wraps
/// `make`, so that read appears in it. A trace that only ever saw the verifier
/// would leave a suppressed attempt on a recipe line unreported, and this
/// assertion is what rules that out.
#[cfg(target_os = "linux")]
fn ensure_recipe_commands_traced(trace: &str) -> Result<()> {
    ensure!(
        trace.contains("KANI_VERSION"),
        "the trace must include the recipe's own read of the pin file, got {trace:?}"
    );
    Ok(())
}

/// Assert the installed verifier answered through the recipe and attempted
/// nothing.
///
/// The fixture's stub is what proves the reader fires; this proves the real
/// executable behaves, which no stub can. It runs only where the caller named
/// one, so a host without Kani still exercises the rest of the boundary.
#[cfg(target_os = "linux")]
fn ensure_installed_query_works(
    run: &support::SandboxRun,
    stdout: &str,
    stderr: &str,
) -> Result<()> {
    let Some(trace) = run.installed_trace.as_deref() else {
        ensure!(
            stdout.contains("installed-exit=unset"),
            "the sandbox must report an unexercised installed verifier, got `{stdout}`"
        );
        return Ok(());
    };
    ensure!(
        stdout.contains("installed-exit=0"),
        "the installed verifier must answer through the recipe inside the boundary, \
         got stdout `{stdout}` and stderr `{stderr}`"
    );
    let findings = trace::attempts(trace);
    ensure!(
        findings.is_empty(),
        "the installed verifier must attempt no write or connection, got {findings:?}"
    );
    Ok(())
}

/// Assert the control trace names both attempts the control discarded.
#[cfg(target_os = "linux")]
fn ensure_control_attempts_traced(trace: &str) -> Result<()> {
    let findings = trace::attempts(trace);
    for expected in ["write-intent", "network address"] {
        ensure!(
            findings.iter().any(|finding| finding.contains(expected)),
            "the control must be reported as a {expected} attempt, got {findings:?}"
        );
    }
    Ok(())
}
