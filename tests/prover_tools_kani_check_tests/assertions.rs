//! The assertions the `kani-check` isolation case makes about its sandbox.
//!
//! Each helper answers one question about a completed run — did the sandbox
//! apply a restriction, did the query attempt nothing, did a control's
//! discarded attempt reach the trace — and reports a failure with the evidence
//! that decided it. They live here rather than beside the case so that neither
//! file outgrows the module size cap; the case itself stays in the crate root.
//!
//! Each helper takes the whole [`SandboxRun`] rather than the stream or trace
//! it reads. The run already holds every one of those, so a helper that took a
//! `&str` would make the caller choose the evidence and the helper unable to
//! say which record it examined. Reading the named field here keeps each
//! assertion's evidence in one place and keeps the signatures uniform.

use crate::support::SandboxRun;
use crate::trace;
use anyhow::{Result, ensure};
use std::borrow::Cow;

/// The reports a sandboxed query must make once the sandbox has applied every
/// restriction: each blocked path, the absence of a usable interface and DNS,
/// and the successful version match.
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

/// Return the run's standard output, replacing any invalid UTF-8.
fn standard_output(run: &SandboxRun) -> Cow<'_, str> {
    String::from_utf8_lossy(&run.output.stdout)
}

/// Return the run's standard error, replacing any invalid UTF-8.
fn standard_error(run: &SandboxRun) -> Cow<'_, str> {
    String::from_utf8_lossy(&run.output.stderr)
}

/// Assert the sandbox reported every restriction it applied.
pub(crate) fn ensure_restrictions_reported(run: &SandboxRun) -> Result<()> {
    let stdout = standard_output(run);
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
pub(crate) fn ensure_home_unwritable(run: &SandboxRun) -> Result<()> {
    let stdout = standard_output(run);
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
pub(crate) fn ensure_only_control_writable(run: &SandboxRun) -> Result<()> {
    let stdout = standard_output(run);
    for line in stdout.lines().filter(|line| line.contains('=')) {
        ensure!(
            line == "control=writable" || !line.ends_with("=writable"),
            "only the control path may be writable, got `{line}` in `{stdout}`"
        );
    }
    Ok(())
}

/// Assert the query's own trace names no attempted write or connection.
pub(crate) fn ensure_query_attempts_nothing(run: &SandboxRun) -> Result<()> {
    let findings = trace::attempts(&run.query_trace);
    ensure!(
        findings.is_empty(),
        "the isolated query must attempt no write or connection, got {findings:?}"
    );
    Ok(())
}

/// Assert the query trace covers the recipe's own commands, not only the
/// verifier.
///
/// The recipe reads the pin file before it runs anything, and the trace wraps
/// `make`, so that read appears in it. A trace that only ever saw the verifier
/// would leave a suppressed attempt on a recipe line unreported, and this
/// assertion is what rules that out.
///
/// The name alone is not evidence: the sandbox passes `KANI_VERSION_FILE=` on
/// the make command line, so the `execve` line carries it whether or not the
/// file was ever opened. The assertion therefore requires a line that both
/// names the pin and is an open call.
pub(crate) fn ensure_recipe_commands_traced(run: &SandboxRun) -> Result<()> {
    ensure!(
        run.query_trace.lines().any(is_pin_read),
        "the trace must include the recipe's own read of the pin file, got {trace:?}",
        trace = run.query_trace
    );
    Ok(())
}

/// Report whether a trace line opens the pin file.
fn is_pin_read(line: &str) -> bool {
    let is_open_call = ["open(", "openat(", "openat2("]
        .iter()
        .any(|call| line.contains(call));
    is_open_call && line.contains("KANI_VERSION")
}

/// Assert the installed verifier answered through the recipe and attempted
/// nothing.
///
/// The fixture's stub is what proves the reader fires; this proves the real
/// executable behaves, which no stub can. It runs only where the caller named
/// one, so a host without Kani still exercises the rest of the boundary.
pub(crate) fn ensure_installed_query_works(run: &SandboxRun) -> Result<()> {
    let stdout = standard_output(run);
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
         got stdout `{stdout}` and stderr `{stderr}`",
        stderr = standard_error(run)
    );
    let findings = trace::attempts(trace);
    ensure!(
        findings.is_empty(),
        "the installed verifier must attempt no write or connection, got {findings:?}"
    );
    Ok(())
}

/// Assert an incomplete installation is refused before the launcher can install.
///
/// `cargo-kani` runs a first-time setup — a download and an extraction — when
/// the pinned payload is absent, so the recipe refuses to invoke it there. This
/// run binds a Kani home with no payload and requires the refusal, then requires
/// the missing-payload trace to hold no attempt at all.
///
/// The refusal is what fails if the guard is removed, not the trace: the run
/// uses the same stub verifier as the others, and a stub answers `--version`
/// without installing anything, so a removed guard would let the run succeed
/// rather than leave a setup attempt to find. The trace's claim is the narrower
/// one that the refusal itself attempted nothing.
pub(crate) fn ensure_missing_payload_refused(run: &SandboxRun) -> Result<()> {
    let stdout = standard_output(run);
    let stderr = standard_error(run);
    // Make normalizes a failed recipe to its own status, so the value is only
    // required to be non-zero rather than to be the recipe's.
    let refused = stdout
        .lines()
        .find_map(|line| line.strip_prefix("missing-exit="))
        .is_some_and(|status| status.trim() != "0");
    ensure!(
        refused,
        "the query must refuse an absent payload with a failure status, \
         got stdout `{stdout}` and stderr `{stderr}`"
    );
    ensure!(
        stderr.contains("category=payload-missing"),
        "the refusal must name the missing payload, got stderr `{stderr}`"
    );
    let findings = trace::attempts(&run.missing_trace);
    ensure!(
        findings.is_empty(),
        "a refused payload must attempt no write or connection, got {findings:?}"
    );
    Ok(())
}

/// Assert a suppressed attempt on a recipe line is traced and still succeeds.
///
/// The verifier control proves the reader fires on a verifier's attempts. A
/// recipe line is the other place an attempt can hide, and a discarded failure
/// there leaves the make run looking exactly like one that never tried. The
/// control recipe therefore discards a write and a directory creation and then
/// reports the pinned version, so `dirty-exit=0` shows success survived while
/// the dirty recipe's trace is the only record of either attempt.
pub(crate) fn ensure_dirty_recipe_attempts_traced(run: &SandboxRun) -> Result<()> {
    let stdout = standard_output(run);
    ensure!(
        stdout.contains("dirty-exit=0"),
        "the control recipe must succeed while discarding its attempts, \
         got stdout `{stdout}` and stderr `{stderr}`",
        stderr = standard_error(run)
    );
    let findings = trace::attempts(&run.dirty_trace);
    ensure!(
        findings
            .iter()
            .any(|finding| finding.contains("write-intent")),
        "a suppressed recipe write must be reported as a write intent, got {findings:?}"
    );
    // `mkdir` has a syscall of its own only on some architectures; aarch64
    // spells the same operation `mkdirat`, so the reported name is not fixed.
    ensure!(
        findings
            .iter()
            .any(|finding| finding.starts_with("`mkdir") && finding.ends_with("changes a path")),
        "a suppressed recipe directory creation must be reported, got {findings:?}"
    );
    Ok(())
}

/// Assert the control trace names both attempts the control discarded.
pub(crate) fn ensure_control_attempts_traced(run: &SandboxRun) -> Result<()> {
    let findings = trace::attempts(&run.control_trace);
    for expected in ["write-intent", "network address"] {
        ensure!(
            findings.iter().any(|finding| finding.contains(expected)),
            "the control must be reported as a {expected} attempt, got {findings:?}"
        );
    }
    Ok(())
}
