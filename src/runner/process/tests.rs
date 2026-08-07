//! Unit and property tests for Ninja process helpers.

use super::super::{NINJA_ENV, NINJA_PROGRAM};
use super::*;
use camino::Utf8PathBuf;
use mockable::MockEnv;
use proptest::prelude::*;
use rstest::{fixture, rstest};
use std::ffi::OsString;
#[cfg(unix)]
use std::path::PathBuf;

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
        ForwardStats::default()
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
