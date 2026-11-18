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
//! This module runs an ignored child copy of the test binary under `strace`,
//! injects `EPERM` for `openat2`, and correlates the trace with the fixtures
//! the child reads. The parent never mutates its own environment: the marker
//! that lets the child run is set on the child `Command` only.
//!
//! `strace` is a Linux tracing tool, and the injected fault is a Linux syscall
//! fault, so the module is compiled on Linux alone. macOS always uses the
//! manual resolver, so its read-policy lane exercises the same policy without
//! this harness.
use std::{fs, path::Path, process::Command, time::Duration};

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use minijinja::ErrorKind;
use tempfile::tempdir;
use test_support::{bounded_process::output_within, fluent::normalize_fluent_isolates};

use super::{
    CONTENTS, DIGEST, FilterCase, HASH, LINECOUNT, Policy, ReadTarget, create_symlink, fallible,
    rejection, render_case, require_real_symlink, write_fixture,
};

/// The file-reading filters, in the order the fixture table exercises them.
const CASES: [FilterCase; 4] = [CONTENTS, LINECOUNT, HASH, DIGEST];

/// Environment variable that lets the ignored child run.
///
/// The child is `#[ignore]`d so an ordinary test run never renders outside the
/// parent's trace; the parent sets this marker on the child `Command` only.
const CHILD_MARKER: &str = "NETSUKE_READ_POLICY_RESOLVER_CHILD";

/// Environment variable that turns an unavailable tracer into a failure.
const STRICT_ENV: &str = "NETSUKE_READ_POLICY_RESOLVER_STRICT";

/// The exact libtest name of the child, as `--exact` must spell it.
const CHILD_TEST: &str =
    "read_policy_filters::resolver_fallback_tests::manual_resolver_child_renders_every_policy";

/// How long the traced child may run before the test fails.
///
/// The child starts a fresh test process and reads a handful of tiny files; the
/// allowance is for a loaded host, not for the work itself.
const CHILD_DEADLINE: Duration = Duration::from_secs(180);

/// What the manual resolver is expected to do with one traced basename.
#[derive(Clone, Copy, Debug)]
enum TracePath {
    /// The final component is a link the no-follow policy refuses.
    RefusedLink,
    /// The final component is a link the opt-in resolves.
    FollowedLink,
    /// The final component is an ordinary file.
    RegularFile,
}

/// One fixture the child reads and the trace must account for.
#[derive(Clone, Debug)]
struct TraceExpectation {
    /// Fixture basename the trace must mention.
    name: String,
    /// What the manual resolver must do with that basename.
    path: TracePath,
}

impl TraceExpectation {
    /// The expectations for `case` and `policy`.
    fn for_symlink(case: FilterCase, policy: Policy) -> Self {
        Self {
            name: link_fixture_name(case, policy),
            path: if policy.follows() {
                TracePath::FollowedLink
            } else {
                TracePath::RefusedLink
            },
        }
    }

    /// The expectation for the regular-file control of `case`.
    fn for_control(case: FilterCase) -> Self {
        Self {
            name: control_fixture_name(case),
            path: TracePath::RegularFile,
        }
    }
}

/// The basename of the symlink fixture for `case` under `policy`.
fn link_fixture_name(case: FilterCase, policy: Policy) -> String {
    format!("link_{}_{}", case.name, policy.suffix())
}

/// The basename of the regular-file control for `case`.
fn control_fixture_name(case: FilterCase) -> String {
    format!("control_{}", case.name)
}

/// Every fixture the traced child reads, in trace-correlation order.
fn trace_expectations() -> Vec<TraceExpectation> {
    let mut expectations = Vec::with_capacity(CASES.len() * (Policy::ALL.len() + 1));
    for case in CASES {
        for policy in Policy::ALL {
            expectations.push(TraceExpectation::for_symlink(case, policy));
        }
        expectations.push(TraceExpectation::for_control(case));
    }
    expectations
}

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

    let trace_text = fs::read_to_string(&trace).context("read the strace output")?;
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

/// Assert that `trace` shows the manual resolver handling `expectation`.
fn assert_trace(trace: &str, expectation: &TraceExpectation) -> Result<()> {
    let quoted = format!("\"{}\"", expectation.name);
    let openat2_refused = trace.lines().any(|line| {
        line.contains("openat2(") && line.contains(&quoted) && line.contains("= -1 EPERM")
    });
    ensure!(
        openat2_refused,
        "the injected fault did not refuse an openat2 for {}:\n{}",
        expectation.name,
        trace_excerpt(trace, &quoted)
    );

    let manual = trace_excerpt(trace, &quoted);
    ensure!(
        trace
            .lines()
            .any(|line| line.contains("openat(") && line.contains(&quoted)),
        "the manual resolver did not open {} after openat2 was refused:\n{manual}",
        expectation.name
    );

    match expectation.path {
        TracePath::RefusedLink => {
            ensure!(
                trace.lines().any(|line| line.contains("openat(")
                    && line.contains(&quoted)
                    && line.contains("= -1 ELOOP")),
                "the manual resolver did not refuse the link {} with ELOOP:\n{manual}",
                expectation.name
            );
            ensure!(
                !trace
                    .lines()
                    .any(|line| line.contains("readlinkat(") && line.contains(&quoted)),
                "the manual resolver dereferenced the refused link {}:\n{manual}",
                expectation.name
            );
        }
        TracePath::FollowedLink => {
            ensure!(
                trace
                    .lines()
                    .any(|line| line.contains("readlinkat(") && line.contains(&quoted)),
                "the opt-in did not dereference the link {}:\n{manual}",
                expectation.name
            );
        }
        TracePath::RegularFile => {
            ensure!(
                trace.lines().any(|line| line.contains("openat(")
                    && line.contains(&quoted)
                    && !line.contains("= -1 ")),
                "the manual resolver did not open the regular file {}:\n{manual}",
                expectation.name
            );
        }
    }
    Ok(())
}

/// Return the trace lines mentioning `needle`, for failure diagnostics.
fn trace_excerpt(trace: &str, needle: &str) -> String {
    let relevant: Vec<&str> = trace.lines().filter(|line| line.contains(needle)).collect();
    if relevant.is_empty() {
        "<no trace lines>".to_owned()
    } else {
        relevant.join("\n")
    }
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
    let output = Command::new("strace")
        .args(["-f", "--kill-on-exit", "-o"])
        .arg(probe_dir.path().join("probe.trace"))
        .args(["-e", "trace=openat2", "-e", "inject=openat2:error=EPERM"])
        .arg("true")
        .output();
    match output {
        Ok(output) if output.status.success() => None,
        Ok(output) => Some(format!(
            "strace cannot inject an openat2 fault: {}",
            String::from_utf8_lossy(&output.stderr).trim()
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

/// Whether the parent asked this process to run as its traced child.
#[expect(
    clippy::disallowed_methods,
    reason = "the marker is set on the child Command because the trace claims the whole process; there is no injected environment in an integration-test binary"
)]
fn is_requested_child() -> bool {
    std::env::var_os(CHILD_MARKER).is_some()
}

/// The child: render every filter under every policy under the manual resolver.
///
/// Run only by [`manual_resolver_enforces_the_no_follow_policy`], through
/// `strace` with `openat2` faulted. It asserts the policy itself; the parent
/// asserts that the manual path was taken.
#[test]
#[ignore = "spawned by manual_resolver_enforces_the_no_follow_policy under strace"]
fn manual_resolver_child_renders_every_policy() -> Result<()> {
    if !is_requested_child() {
        return Ok(());
    }

    let (_temp, root) = fallible::filter_workspace()?;
    for case in CASES {
        let control = write_fixture(&root, &control_fixture_name(case), b"data")?;
        let rendered = render_case(
            case,
            "resolver_control",
            ReadTarget::new(&root, &control, 1024),
            &case.template(),
        )?
        .with_context(|| format!("{}: regular-file control", case.name))?;
        ensure!(
            rendered == case.followed,
            "{}: expected {} for the regular-file control but rendered {rendered}",
            case.name,
            case.followed
        );

        for policy in Policy::ALL {
            let link = create_symlink(&root, &link_fixture_name(case, policy), "file")
                .with_context(|| format!("{}: create the {:?} link", case.name, policy))?;
            require_real_symlink(&root, &link)?;
            render_policy_case(&root, case, policy, &link)?;
        }
    }
    Ok(())
}

/// Render one symlink case under `policy` and assert the documented outcome.
fn render_policy_case(
    root: &Utf8Path,
    case: FilterCase,
    policy: Policy,
    link: &Utf8Path,
) -> Result<()> {
    let rendered = render_case(
        case,
        &format!("resolver_{}", policy.suffix()),
        ReadTarget::new(root, link, 1024),
        &policy.template(case),
    )?;

    if policy.follows() {
        let output =
            rendered.with_context(|| format!("{}: render under {:?}", case.name, policy))?;
        ensure!(
            output == case.followed,
            "{}: the opt-in should read the link target under the manual resolver; \
             expected {} but rendered {output}",
            case.name,
            case.followed
        );
        return Ok(());
    }

    let err = rejection(case, rendered)?;
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "{}: should report InvalidOperation under {:?} but was {:?}",
        case.name,
        policy,
        err.kind()
    );
    let message = normalize_fluent_isolates(&err.to_string());
    ensure!(
        message.contains("not a regular file") || message.contains("symbolic links"),
        "{}: error should explain the rejection under {:?}: {message}",
        case.name,
        policy
    );
    Ok(())
}
