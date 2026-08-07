//! Unit tests for injected executable-search environment capture.

use super::*;
use crate::test_tracing_capture::with_test_subscriber;
use mockable::MockEnv;
use tracing::level_filters::LevelFilter;

/// The exact warning capture emits for a non-UTF-8 switch value. Asserted
/// verbatim: without it the test passes even when the message is replaced
/// wholesale, which is precisely what happened before.
const NOT_UNICODE_WARNING: &str = "workspace fallback disabled because env var is not valid UTF-8";

#[test]
fn capture_uses_the_injected_path_provider() {
    let cwd = Utf8Path::new("/workspace");
    let configured = OsString::from("/configured/bin");
    let expected = configured.clone();
    let mut env = MockEnv::new();
    env.expect_os_string()
        .withf(|key| key == "PATH")
        .once()
        .return_once(move |_| Some(configured));
    // Capture also reads the workspace switch through the same provider;
    // pinning the key keeps that read observable rather than wildcarded.
    env.expect_raw()
        .withf(|key| key == WORKSPACE_FALLBACK_ENV)
        .once()
        .return_const(Err(std::env::VarError::NotPresent));

    let snapshot = EnvSnapshot::capture_with_env(Some(cwd), None, &env)
        .expect("injected PATH should produce an environment snapshot");

    assert_eq!(snapshot.raw_path, Some(expected));
    assert_eq!(
        snapshot.resolved_dirs(CwdMode::Never),
        [Utf8Path::new("/configured/bin")]
    );
}

/// A non-UTF-8 switch value must warn exactly once, at capture.
///
/// The classifier is pure, so the diagnostic lives at the boundary where
/// the ambient reading is taken; consulting the switch afterwards adds
/// nothing. Silently switching workspace search off would leave a user
/// whose variable is mis-encoded with commands mysteriously unresolved
/// and no indication why.
#[test]
fn capture_warns_once_for_a_non_utf8_workspace_switch() {
    let mut env = MockEnv::new();
    env.expect_os_string()
        .withf(|key| key == "PATH")
        .once()
        .return_once(|_| Some(OsString::from("/configured/bin")));
    env.expect_raw()
        .withf(|key| key == WORKSPACE_FALLBACK_ENV)
        .once()
        .return_const(Err(std::env::VarError::NotUnicode(OsString::from(
            "ignored",
        ))));

    let (enabled, events) = with_test_subscriber(LevelFilter::WARN, |captured| {
        let snapshot = EnvSnapshot::capture_with_env(Some(Utf8Path::new("/workspace")), None, &env)
            .expect("a non-UTF-8 switch value should not fail capture");
        (snapshot.workspace_fallback_enabled(), captured.snapshot())
    });

    assert!(!enabled, "a non-UTF-8 value must disable the fallback");
    // The subscriber admits WARN and above, so the level is asserted by
    // the filter; requiring exactly one event proves the warning fires at
    // capture and is not repeated when the switch is consulted.
    assert_eq!(
        events.len(),
        1,
        "expected exactly one warning, got {events:?}"
    );
    let expectation =
        format!("a warning naming {WORKSPACE_FALLBACK_ENV} carrying {NOT_UNICODE_WARNING:?}");
    assert!(
        events.iter().any(|event| {
            event.contains(WORKSPACE_FALLBACK_ENV) && event.contains(NOT_UNICODE_WARNING)
        }),
        "expected {expectation}, got {events:?}"
    );
}
