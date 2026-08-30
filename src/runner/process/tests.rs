//! Unit and property tests for Ninja process helpers.

use super::child_exit::finalize_streaming;
#[cfg(unix)]
use super::command_list_telemetry::COMMAND_LIST_FAILURE_DURATION;
use super::streaming::ForwardStats;
use super::*;
use crate::test_tracing_capture::with_test_subscriber;
use camino::{Utf8Path, Utf8PathBuf};
#[cfg(unix)]
use metrics_util::{
    MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};
#[cfg(unix)]
use monotony::StdMonotonicClock;
#[cfg(unix)]
use monotony::test_util::FixedMonotonicClock;
use std::path::Path;
#[cfg(unix)]
use std::process::Stdio;
use std::thread;
#[cfg(unix)]
use std::time::Duration;
use tracing_subscriber::filter::LevelFilter;

#[cfg(unix)]
#[path = "public_ninja_execution_tests.rs"]
mod public_ninja_execution_tests;

/// Open a capability directory rooted at an owned UTF-8 temporary directory.
pub(super) fn temporary_dir(temp: &tempfile::TempDir) -> anyhow::Result<cap_std::fs_utf8::Dir> {
    let path = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("temporary directory is not UTF-8: {}", path.display()))?;
    cap_std::fs_utf8::Dir::open_ambient_dir(path, cap_std::ambient_authority()).map_err(Into::into)
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

/// The child process runs in the requested working directory.
///
/// The fake Ninja records its effective current directory to a file whose path
/// travels via an injected environment variable (mirroring the shared
/// run-marker pattern), so no path is interpolated into the shell script. The
/// recorded value is compared against the canonicalised working directory,
/// which is the path the process layer passes to the child.
#[cfg(unix)]
#[test]
fn run_ninja_with_runs_in_the_requested_working_directory() -> anyhow::Result<()> {
    use test_support::exec::write_exec_with_content;

    let working_dir = tempfile::tempdir()?;
    // `configure_ninja_base` canonicalises the build file before spawning, so
    // the manifest must already exist in the working directory.
    test_support::fs::write(working_dir.path().join("build.ninja"), "# empty manifest\n")?;
    let record_dir = tempfile::tempdir()?;
    let observed_file = record_dir.path().join("observed-cwd");
    let fake_ninja = write_exec_with_content(
        record_dir.path(),
        "fake-ninja",
        "#!/bin/sh\npwd > \"${RECORD_CWD_TO}\"\nexit 0\n",
    )?;

    let working_dir_utf8 = Utf8PathBuf::from_path_buf(working_dir.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("tempdir path is not UTF-8: {}", path.display()))?;
    let utf8_fake_ninja = Utf8PathBuf::from_path_buf(fake_ninja)
        .map_err(|path| anyhow::anyhow!("fake Ninja path is not UTF-8: {}", path.display()))?;
    let build_file = working_dir_utf8.join("build.ninja");
    let options = NinjaProcessOptions {
        working_dir: Some(working_dir_utf8),
        ..NinjaProcessOptions::default()
    };
    let targets = BuildTargets::default();
    let env = CommandEnv::inherit().with_var("RECORD_CWD_TO", &observed_file);
    run_ninja_with(&NinjaBuildRequest {
        program: &utf8_fake_ninja,
        options: &options,
        build_file: &build_file,
        targets: &targets,
        env: &env,
        stderr_mode: StderrMode::Forward,
    })?;

    let expected = working_dir.path().canonicalize()?;
    let recorded = test_support::fs::read_to_string(&observed_file)?;
    anyhow::ensure!(
        Path::new(recorded.trim()) == expected,
        "Ninja should run in the requested working directory: expected {expected:?}, \
         recorded {recorded:?}"
    );
    Ok(())
}

/// Spawning a missing Ninja emits a spawn-failure warning whose
/// `suppress_stderr` field follows the request's explicit `stderr_mode`.
#[test]
fn spawn_failure_logging_honours_explicit_stderr_mode() {
    let cases = [
        (true, StderrMode::Forward, "suppress_stderr=false"),
        (false, StderrMode::Suppress, "suppress_stderr=true"),
    ];
    for (json, mode, expected_field) in cases {
        let options = NinjaProcessOptions::default();
        let targets = BuildTargets::default();
        let events = with_test_subscriber(LevelFilter::WARN, |captured| {
            let result = run_ninja_with(&NinjaBuildRequest {
                program: Utf8Path::new("netsuke-test-missing-ninja"),
                options: &options,
                build_file: Utf8Path::new("build.ninja"),
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
            "exactly one warning should be captured for {mode:?} with input={json}, \
             got: {events:?}"
        );
        // The single captured warning is the spawn failure; inspect it alone so a
        // stray event carrying the expected field cannot mask a missing frame.
        let event = events
            .first()
            .expect("the exactly-one-warning assertion above guarantees a first event");
        assert!(
            event.contains("failure_category=\"spawn\"") && event.contains(expected_field),
            "the captured warning should be a spawn failure recording {expected_field} for \
             {mode:?} with input={json}, got: {events:?}"
        );
    }
}
