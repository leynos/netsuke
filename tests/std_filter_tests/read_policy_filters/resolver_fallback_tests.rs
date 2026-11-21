//! Force cap-std's manual path resolver and prove the read policy holds there.
//!
//! Native Linux resolves inside `openat2`, where the raw `O_NOFOLLOW` flag
//! refuses a final symlink on its own. macOS, Linux before 5.6 and any host
//! whose seccomp policy refuses `openat2` resolve one component at a time
//! instead, and that resolver decides from cap-std's logical `FollowSymlinks`
//! policy. Setting only the operating-system flag left the logical policy at
//! its `Yes` default, so the manual resolver dereferenced the final link and
//! the bounded readers consumed the target despite `follow_symlinks=false`.
//!
//! This module runs an ignored child copy of the test binary under `strace`
//! and injects `EPERM` for `openat2`. The parent never mutates its own
//! environment: the marker that lets the child run is set on the child
//! `Command` only. The child's own assertions live in `resolver_child_tests`,
//! and what the resulting trace must show lives in `resolver_trace`, so this
//! module holds only the spawn, the availability probe and the deadline.
//!
//! `strace` is a Linux tracing tool, and the injected fault is a Linux syscall
//! fault, so the module is compiled on Linux alone. macOS always uses the
//! manual resolver, so its read-policy lane exercises the same policy without
//! this harness.
use std::{path::Path, process::Command, time::Duration};

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use tempfile::tempdir;
use test_support::bounded_process::output_within;

use super::resolver_trace::{assert_trace, trace_expectations};
use super::{CONTENTS, DIGEST, FilterCase, HASH, LINECOUNT};

/// The file-reading filters, in the order the fixture table exercises them.
pub(super) const CASES: [FilterCase; 4] = [CONTENTS, LINECOUNT, HASH, DIGEST];

/// Environment variable that lets the ignored child run.
///
/// The child is `#[ignore]`d so an ordinary test run never renders outside the
/// parent's trace; the parent sets this marker on the child `Command` only.
pub(super) const CHILD_MARKER: &str = "NETSUKE_READ_POLICY_RESOLVER_CHILD";

/// Environment variable that turns an unavailable tracer into a failure.
const STRICT_ENV: &str = "NETSUKE_READ_POLICY_RESOLVER_STRICT";

/// The exact libtest name of the child, as `--exact` must spell it.
const CHILD_TEST: &str =
    "read_policy_filters::resolver_child_tests::manual_resolver_child_renders_every_policy";

/// How long the traced child may run before the test fails.
///
/// The child starts a fresh test process and reads a handful of tiny files; the
/// allowance is for a loaded host, not for the work itself.
const CHILD_DEADLINE: Duration = Duration::from_secs(180);

/// Prove the no-follow policy under the manual resolver.
///
/// The test is a harness rather than a policy assertion: the policy assertions
/// live in the child, and this case proves the child ran under a resolver that
/// had to fall back. Every fixture basename must show a refused `openat2`
/// followed by the manual `openat` path, so a trace that missed the fixtures
/// cannot report green.
#[test]
fn manual_resolver_enforces_the_no_follow_policy() -> Result<()> {
    if let Some(reason) = fallback_unavailable() {
        return skip(&reason);
    }

    let trace_dir = tempdir().context("create the resolver trace directory")?;
    let trace = trace_dir.path().join("resolver.strace");
    let exe = std::env::current_exe().context("locate the running test binary")?;

    let mut command = traced_child(&exe, &trace);
    let output = output_within(&mut command, CHILD_DEADLINE)
        .context("run the manual-resolver child under strace")?;
    ensure!(
        output.status.success(),
        "the manual-resolver child failed with {}:\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // The trace is read through a capability on its own directory rather than
    // an ambient path, as the repository's filesystem policy requires.
    let trace_root = Dir::open_ambient_dir(trace_dir.path(), ambient_authority())
        .context("open the resolver trace directory")?;
    let trace_text = trace_root
        .read_to_string("resolver.strace")
        .context("read the strace output")?;
    for expectation in trace_expectations() {
        assert_trace(&trace_text, &expectation)?;
    }
    Ok(())
}

/// Build the `strace` invocation that runs the ignored child.
///
/// `--kill-on-exit` is what keeps a deadline kill from leaving the tracee
/// behind. It acts only on tracees `strace` itself knows about, and
/// `output_within` can kill only its direct child, so a grandchild the test
/// binary might start would outlive a deadline. The child reads fixtures and
/// starts none, which is why the limitation is acceptable here.
fn traced_child(exe: &Path, trace: &Path) -> Command {
    let mut command = Command::new("strace");
    command
        .args(["-f", "--kill-on-exit", "-o"])
        .arg(trace)
        .args(["-e", "trace=openat2,openat,readlinkat"])
        .args(["-e", "inject=openat2:error=EPERM"])
        .arg(exe)
        .args(["--exact", CHILD_TEST, "--ignored", "--nocapture"])
        // Child-scoped, as the environment mandate requires: the parent's own
        // environment is untouched.
        .env(CHILD_MARKER, "1");
    command
}

/// Report why the manual-resolver harness cannot run, if it cannot.
///
/// Fault injection is a `strace` feature, so a host without `strace`, or with a
/// build too old to name `openat2` and inject an error for it, cannot force
/// the fallback. The probe runs the same option shape on a throwaway trace so
/// an unsupported option is reported as an unavailable prerequisite rather than
/// failing as though the policy had regressed.
fn fallback_unavailable() -> Option<String> {
    match Command::new("strace").arg("-V").output() {
        Ok(output) if output.status.success() => {}
        Ok(output) => return Some(format!("strace -V exited with {}", output.status)),
        Err(error) => return Some(format!("strace is not runnable: {error}")),
    }

    let probe_dir = match tempdir() {
        Ok(dir) => dir,
        Err(error) => return Some(format!("cannot create the strace probe directory: {error}")),
    };
    let probe = Command::new("strace")
        .args(["-f", "--kill-on-exit", "-o"])
        .arg(probe_dir.path().join("probe.trace"))
        .args(["-e", "trace=openat2", "-e", "inject=openat2:error=EPERM"])
        .arg("true")
        .output();
    match probe {
        Ok(ran) if ran.status.success() => None,
        Ok(ran) => Some(format!(
            "strace cannot inject an openat2 fault: {}",
            String::from_utf8_lossy(&ran.stderr).trim()
        )),
        Err(error) => Some(format!("the strace fault probe is not runnable: {error}")),
    }
}

/// Report a skipped harness, unless strict mode forbids skipping.
///
/// # Errors
///
/// Returns an error when [`STRICT_ENV`] is `1`, so a lane that requires the
/// fallback test fails rather than passing on a host that could not run it.
#[expect(
    clippy::disallowed_methods,
    reason = "this is the single site that reads the strict-mode switch; the test binary has no injected environment to consult"
)]
#[expect(
    clippy::print_stderr,
    reason = "a skipped prerequisite must be visible in the captured test output"
)]
fn skip(reason: &str) -> Result<()> {
    let strict = std::env::var_os(STRICT_ENV).is_some_and(|value| value == "1");
    ensure!(
        !strict,
        "the read-policy manual resolver could not be exercised: {reason} \
         (strict mode requires the fallback test to run for real)"
    );
    eprintln!("skipped: read-policy manual resolver — {reason}");
    Ok(())
}
