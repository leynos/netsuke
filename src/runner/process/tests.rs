//! Unit and property tests for Ninja process helpers.

use super::super::{NINJA_ENV, NINJA_PROGRAM};
use super::child_exit::finalize_streaming;
#[cfg(unix)]
use super::command_list_telemetry::COMMAND_LIST_FAILURE_DURATION;
use super::streaming::ForwardStats;
use super::*;
use crate::cli::Cli;
use crate::test_tracing_capture::with_test_subscriber;
use camino::Utf8PathBuf;
#[cfg(unix)]
use metrics_util::{
    MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};
use mockable::MockEnv;
#[cfg(unix)]
use monotony::{StdMonotonicClock, test_util::FixedMonotonicClock};
use proptest::prelude::*;
use rstest::{fixture, rstest};
use std::ffi::OsString;
use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;
#[cfg(unix)]
use std::process::Stdio;
use std::thread;
#[cfg(unix)]
use std::time::Duration;
use tracing_subscriber::filter::LevelFilter;

/// A `MockEnv` answering exactly one `os_string` read of `NETSUKE_NINJA`.
///
/// The key expectation is part of the contract (#488): a resolver that reads
/// any other variable, or reads more than once, fails these tests rather than
/// silently consulting something else. Consumers override the answer with
/// `#[with(...)]`; the `#[default(None)]` parameter models "variable unset".
#[fixture]
fn ninja_env(#[default(None)] value: Option<OsString>) -> MockEnv {
    let mut env = MockEnv::new();
    env.expect_os_string()
        .times(1)
        .withf(|key| key == NINJA_ENV)
        .return_const(value);
    env
}

#[rstest]
fn resolve_ninja_program_utf8_prefers_env_override(
    #[with(Some(OsString::from("/opt/ninja")))] ninja_env: MockEnv,
) {
    let resolved = resolve_ninja_program_utf8_with(&ninja_env);
    assert_eq!(resolved, Utf8PathBuf::from("/opt/ninja"));
}

#[rstest]
fn resolve_ninja_program_utf8_defaults_without_override(ninja_env: MockEnv) {
    let resolved = resolve_ninja_program_utf8_with(&ninja_env);
    assert_eq!(resolved, Utf8PathBuf::from(NINJA_PROGRAM));
}

#[rstest]
fn resolve_ninja_program_utf8_defaults_for_empty_override(
    #[with(Some(OsString::new()))] ninja_env: MockEnv,
) {
    let resolved = resolve_ninja_program_utf8_with(&ninja_env);
    assert_eq!(resolved, Utf8PathBuf::from(NINJA_PROGRAM));
}

#[cfg(unix)]
#[rstest]
fn resolve_ninja_program_utf8_ignores_invalid_utf8_override(
    #[with(Some(invalid_utf8_override()))] ninja_env: MockEnv,
) {
    let resolved = resolve_ninja_program_utf8_with(&ninja_env);
    assert_eq!(resolved, Utf8PathBuf::from(NINJA_PROGRAM));
}

/// A non-UTF-8 override value; the leading `0xff` byte is never valid UTF-8.
#[cfg(unix)]
fn invalid_utf8_override() -> OsString {
    use std::os::unix::ffi::OsStringExt;

    OsString::from_vec(vec![0xff, b'n', b'i', b'n', b'j', b'a'])
}

/// The platform-path variant shares the UTF-8 resolution and conversion.
#[rstest]
fn resolve_ninja_program_with_converts_the_resolved_path(
    #[with(Some(OsString::from("/opt/ninja")))] ninja_env: MockEnv,
) {
    let resolved = resolve_ninja_program_with(&ninja_env);
    assert_eq!(resolved, std::path::PathBuf::from("/opt/ninja"));
}

// `proptest!` owns the generated function signature, so rstest cannot inject
// the fixture here. Calling the fixture function directly keeps the one-read,
// exact-key contract identical to the injected cases without weakening the
// property's input coverage.
proptest! {
    #[test]
    fn resolve_ninja_program_utf8_matches_utf8_env_invariant(
        override_value in prop::option::of(".*")
    ) {
        let env_value = override_value.clone().map(OsString::from);
        let expected = match override_value {
            Some(value) if !value.is_empty() => Utf8PathBuf::from(value),
            _ => Utf8PathBuf::from(NINJA_PROGRAM),
        };

        let resolved = resolve_ninja_program_utf8_with(&ninja_env(env_value));

        prop_assert_eq!(resolved, expected);
    }
}

#[test]
fn finalize_streaming_joins_stderr_thread_when_wait_fails() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::Duration;

    // The thread signals completion only after a short delay, so if
    // `finalize_streaming` failed to join it, the flag would still be unset
    // when the buggy path returned early on the wait error.
    let joined = Arc::new(AtomicBool::new(false));
    let worker_flag = Arc::clone(&joined);
    let err_handle = thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        worker_flag.store(true, Ordering::SeqCst);
        (ForwardStats::default(), None)
    });

    let wait_result = Err(io::Error::other("simulated wait failure"));
    let result = finalize_streaming(wait_result, ForwardStats::default(), err_handle);

    assert!(
        result.is_err(),
        "a wait() failure must propagate after cleanup"
    );
    assert!(
        joined.load(Ordering::SeqCst),
        "the stderr forwarding thread must be joined even when wait() fails"
    );
}

#[cfg(unix)]
#[test]
fn command_list_failure_duration_uses_the_injected_monotonic_clock() {
    let duration = Duration::from_millis(7);
    let clock = FixedMonotonicClock::with_elapsed(duration);
    let mut command = Command::new("sh");
    command
        .args([
            "-c",
            concat!(
                "printf '%s\\n' 'netsuke command-list failure: action ",
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef, entry 2' >&2; ",
                "exit 1"
            ),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    let execution = CommandExecutionContext {
        operation: "build",
        stderr_mode: StderrMode::Suppress,
        captures_ninja_failure_output: false,
        clock: &clock,
    };
    let result = metrics::with_local_recorder(&recorder, || {
        run_command_and_stream_with_context(command, None, &execution)
    });

    assert!(result.is_err(), "the attributed command should fail");
    let snapshot = snapshotter.snapshot().into_vec();
    let recorded_durations = snapshot
        .iter()
        .filter(|(key, _, _, value)| {
            key.kind() == MetricKind::Histogram
                && key.key().name() == COMMAND_LIST_FAILURE_DURATION
                && matches!(
                    value,
                    DebugValue::Histogram(samples)
                        if samples.as_slice() == [duration.as_secs_f64()]
                )
        })
        .count();
    assert_eq!(
        recorded_durations, 1,
        "the failure duration must use the injected clock exactly once"
    );
}

#[cfg(unix)]
#[test]
fn large_stdout_cannot_supply_command_list_attribution() -> anyhow::Result<()> {
    let mut command = Command::new("sh");
    command
        .args([
            "-c",
            concat!(
                "yes x | head -c 262144; ",
                "printf '%s\\n' 'netsuke command-list failure: action ",
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef, entry 2'; ",
                "exit 1"
            ),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let execution = CommandExecutionContext {
        operation: "build",
        stderr_mode: StderrMode::Suppress,
        // Only a Ninja build can relay a subcommand's stderr through stdout.
        // An arbitrary command's large stdout must be forwarded untouched.
        captures_ninja_failure_output: false,
        clock: &StdMonotonicClock,
    };

    let Err(error) = run_command_and_stream_with_context(command, None, &execution) else {
        anyhow::bail!("a failing command should return an error");
    };

    if error
        .to_string()
        .contains("netsuke command-list failure: action ")
    {
        anyhow::bail!("stdout must not supply command-list attribution: {error}");
    }
    Ok(())
}

// As above, the fixture is called directly because `proptest!` generates the
// function signature and leaves no parameter for rstest to inject.
#[cfg(unix)]
proptest! {
    #[test]
    fn resolve_ninja_program_utf8_falls_back_for_non_utf8_env_values(
        bytes in prop::collection::vec(any::<u8>(), 0..32)
    ) {
        use std::os::unix::ffi::OsStringExt;

        let env_value = OsString::from_vec(bytes);
        let expected = if env_value.as_os_str().is_empty() {
            Utf8PathBuf::from(NINJA_PROGRAM)
        } else {
            Utf8PathBuf::from_path_buf(PathBuf::from(env_value.clone()))
                .unwrap_or_else(|_| Utf8PathBuf::from(NINJA_PROGRAM))
        };

        let resolved = resolve_ninja_program_utf8_with(&ninja_env(Some(env_value)));

        prop_assert_eq!(resolved, expected);
    }
}

/// Spawning a missing Ninja emits a spawn-failure warning whose
/// `suppress_stderr` field follows the request's explicit `stderr_mode`, not
/// the request's `cli.json` state. The mismatch in each case proves the process
/// layer consumes the policy field and does not re-derive it from CLI JSON.
#[test]
fn spawn_failure_logging_honours_explicit_stderr_mode() {
    let cases = [
        (true, StderrMode::Forward, "suppress_stderr=false"),
        (false, StderrMode::Suppress, "suppress_stderr=true"),
    ];
    for (json, mode, expected_field) in cases {
        let cli = Cli {
            json,
            ..Cli::default()
        };
        let targets = BuildTargets::default();
        let events = with_test_subscriber(LevelFilter::WARN, |captured| {
            let result = run_ninja_with(&NinjaBuildRequest {
                program: Path::new("netsuke-test-missing-ninja"),
                cli: &cli,
                build_file: Path::new("build.ninja"),
                targets: &targets,
                env: &CommandEnv::inherit(),
                stderr_mode: mode,
            });
            assert!(
                result.is_err(),
                "spawning a missing Ninja should fail before any forwarding"
            );
            captured.snapshot()
        });
        assert_eq!(
            events.len(),
            1,
            "exactly one warning should be captured for {mode:?} with cli.json={json}, \
             got: {events:?}"
        );
        // The single captured warning is the spawn failure; inspect it alone so a
        // stray event carrying the expected field cannot mask a missing frame.
        assert!(
            events[0].contains("failure_category=\"spawn\"") && events[0].contains(expected_field),
            "the captured warning should be a spawn failure recording {expected_field} for \
             {mode:?} with cli.json={json}, got: {events:?}"
        );
    }
}
