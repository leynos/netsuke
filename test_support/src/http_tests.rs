//! Unit tests for the lightweight test HTTP server.

use super::{
    ENV_HTTP_ACCEPT_TIMEOUT_MS, ENV_HTTP_POLL_INTERVAL_MS, ENV_HTTP_READ_TIMEOUT_MS,
    HttpServerConfig, accept_connection, duration_from_env, take_duration_warnings,
};

use crate::{EnvVarGuard, env_lock::EnvLock};
use std::{
    net::TcpListener,
    panic,
    time::{Duration, Instant},
};

#[test]
fn from_env_applies_overrides() {
    let _lock = EnvLock::acquire();
    assert!(
        take_duration_warnings().is_empty(),
        "warnings buffer should start empty"
    );
    let accept = EnvVarGuard::set(ENV_HTTP_ACCEPT_TIMEOUT_MS, "1500");
    let read = EnvVarGuard::set(ENV_HTTP_READ_TIMEOUT_MS, "750");
    let poll = EnvVarGuard::set(ENV_HTTP_POLL_INTERVAL_MS, "25");

    let config = HttpServerConfig::from_env();
    assert_eq!(config.accept_timeout, Duration::from_millis(1500));
    assert_eq!(config.read_timeout, Duration::from_millis(750));
    assert_eq!(config.poll_interval, Duration::from_millis(25));
    assert!(
        take_duration_warnings().is_empty(),
        "no warnings expected for valid overrides"
    );

    drop(poll);
    drop(read);
    drop(accept);
}

#[test]
fn from_env_clamps_zero_poll_interval() {
    let _lock = EnvLock::acquire();
    assert!(
        take_duration_warnings().is_empty(),
        "warnings buffer should start empty"
    );
    let poll = EnvVarGuard::set(ENV_HTTP_POLL_INTERVAL_MS, "0");

    let config = HttpServerConfig::from_env();
    assert_eq!(config.poll_interval, Duration::from_millis(1));
    assert!(
        take_duration_warnings().is_empty(),
        "parsing a zero poll interval should not warn",
    );

    drop(poll);
}

#[test]
fn duration_from_env_returns_default_for_missing() {
    let _lock = EnvLock::acquire();
    assert!(
        take_duration_warnings().is_empty(),
        "warnings buffer should start empty"
    );
    let guard = EnvVarGuard::remove(ENV_HTTP_ACCEPT_TIMEOUT_MS);
    let duration = duration_from_env(ENV_HTTP_ACCEPT_TIMEOUT_MS, Duration::from_secs(3));
    assert_eq!(duration, Duration::from_secs(3));
    assert!(
        take_duration_warnings().is_empty(),
        "missing variables should not log warnings"
    );
    drop(guard);
}

#[test]
fn duration_from_env_reports_invalid_values() {
    let _lock = EnvLock::acquire();
    assert!(
        take_duration_warnings().is_empty(),
        "warnings buffer should start empty"
    );
    let guard = EnvVarGuard::set(ENV_HTTP_ACCEPT_TIMEOUT_MS, "not-a-number");
    let duration = duration_from_env(ENV_HTTP_ACCEPT_TIMEOUT_MS, Duration::from_secs(3));
    assert_eq!(duration, Duration::from_secs(3));
    let warnings = take_duration_warnings();
    assert_eq!(warnings.len(), 1);
    assert!(
        warnings[0].contains(ENV_HTTP_ACCEPT_TIMEOUT_MS),
        "warning should mention the variable name"
    );
    assert!(
        warnings[0].contains("not-a-number"),
        "warning should include the invalid value"
    );
    drop(guard);
}

#[test]
fn duration_from_env_trims_whitespace() {
    let _lock = EnvLock::acquire();
    assert!(
        take_duration_warnings().is_empty(),
        "warnings buffer should start empty"
    );
    let guard = EnvVarGuard::set(ENV_HTTP_READ_TIMEOUT_MS, "  2500  ");
    let duration = duration_from_env(ENV_HTTP_READ_TIMEOUT_MS, Duration::from_secs(3));
    assert_eq!(duration, Duration::from_millis(2500));
    assert!(
        take_duration_warnings().is_empty(),
        "whitespace-only padding should not trigger warnings",
    );
    drop(guard);
}

#[test]
fn accept_connection_respects_accept_timeout() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
    listener
        .set_nonblocking(true)
        .expect("set listener non-blocking");

    let accept_timeout = Duration::from_millis(20);
    let poll_interval = Duration::from_millis(200);
    let start = Instant::now();
    let deadline = start + accept_timeout;

    let result = panic::catch_unwind(|| {
        let _ = accept_connection(&listener, deadline, poll_interval, accept_timeout);
    });
    let panic_payload = result.expect_err("accept_connection should panic when no client connects");

    let elapsed = start.elapsed();
    assert!(
        elapsed >= accept_timeout,
        "panic should not occur before the accept timeout (elapsed {:?}, timeout {:?})",
        elapsed,
        accept_timeout,
    );
    assert!(
        elapsed <= accept_timeout + poll_interval + Duration::from_millis(50),
        "panic overshot accept timeout by more than one poll interval: elapsed={:?}, accept_timeout={:?}, poll_interval={:?}",
        elapsed,
        accept_timeout,
        poll_interval,
    );

    let panic_ref = panic_payload.as_ref();
    let panic_text = panic_ref
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            panic_ref
                .downcast_ref::<&'static str>()
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| format!("{panic_payload:?}"));
    assert!(
        panic_text.contains(&format!("accept_timeout={:?}", accept_timeout)),
        "panic message should embed the accept timeout: {panic_text}",
    );
    assert!(
        panic_text.contains(&format!("poll_interval={:?}", poll_interval)),
        "panic message should embed the poll interval: {panic_text}",
    );
}
