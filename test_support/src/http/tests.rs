//! Unit tests for the HTTP fixture implementation in the parent module.
//!
//! These tests exercise timeout configuration, connection acceptance, and
//! warning capture without exposing test-only helpers through `http`'s public
//! interface.

use super::{
    ENV_HTTP_ACCEPT_TIMEOUT_MS, ENV_HTTP_POLL_INTERVAL_MS, ENV_HTTP_READ_TIMEOUT_MS,
    HttpServerConfig, accept_connection, duration_from_env, take_duration_warnings,
};

use mockable::MockEnv;
use rstest::{fixture, rstest};
use std::{
    collections::HashMap,
    net::TcpListener,
    panic,
    time::{Duration, Instant},
};

fn fixture_env(entries: &[(&str, &str)]) -> MockEnv {
    let values: HashMap<String, String> = entries
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect();
    let mut env = MockEnv::new();
    env.expect_raw().returning(move |key| {
        values
            .get(key)
            .cloned()
            .ok_or(std::env::VarError::NotPresent)
    });
    env
}

#[fixture]
fn empty_duration_warnings() -> EmptyDurationWarnings {
    EmptyDurationWarnings {
        started_empty: take_duration_warnings().is_empty(),
    }
}

struct EmptyDurationWarnings {
    started_empty: bool,
}

impl EmptyDurationWarnings {
    fn take(&self) -> Vec<String> {
        assert!(self.started_empty, "warnings buffer should start empty");
        take_duration_warnings()
    }
}

#[derive(Clone, Copy)]
struct DurationCase {
    key: &'static str,
    value: Option<&'static str>,
    expected: Duration,
    /// Bounded parse-failure text expected in the warning, if it should warn.
    ///
    /// Deliberately not the offending value: the warning redacts it, so
    /// asserting on a category is what keeps that redaction honest.
    expected_warning_error: Option<&'static str>,
}

#[test]
fn from_env_applies_overrides() {
    assert!(
        take_duration_warnings().is_empty(),
        "warnings buffer should start empty"
    );
    let env = fixture_env(&[
        (ENV_HTTP_ACCEPT_TIMEOUT_MS, "1500"),
        (ENV_HTTP_READ_TIMEOUT_MS, "750"),
        (ENV_HTTP_POLL_INTERVAL_MS, "25"),
    ]);

    let config = HttpServerConfig::from_env_provider(&env);
    assert_eq!(config.accept_timeout, Duration::from_millis(1500));
    assert_eq!(config.read_timeout, Duration::from_millis(750));
    assert_eq!(config.poll_interval, Duration::from_millis(25));
    assert!(
        take_duration_warnings().is_empty(),
        "no warnings expected for valid overrides"
    );
}

#[test]
fn from_env_clamps_zero_poll_interval() {
    assert!(
        take_duration_warnings().is_empty(),
        "warnings buffer should start empty"
    );
    let env = fixture_env(&[(ENV_HTTP_POLL_INTERVAL_MS, "0")]);

    let config = HttpServerConfig::from_env_provider(&env);
    assert_eq!(config.poll_interval, Duration::from_millis(1));
    assert!(
        take_duration_warnings().is_empty(),
        "parsing a zero poll interval should not warn",
    );
}

#[rstest]
#[case::missing(DurationCase {
    key: ENV_HTTP_ACCEPT_TIMEOUT_MS,
    value: None,
    expected: Duration::from_secs(3),
    expected_warning_error: None,
})]
#[case::invalid(DurationCase {
    key: ENV_HTTP_ACCEPT_TIMEOUT_MS,
    value: Some("not-a-number"),
    expected: Duration::from_secs(3),
    expected_warning_error: Some("invalid digit"),
})]
#[case::empty(DurationCase {
    key: ENV_HTTP_ACCEPT_TIMEOUT_MS,
    value: Some(""),
    expected: Duration::from_secs(3),
    expected_warning_error: Some("cannot parse integer from empty string"),
})]
#[case::whitespace_padded(DurationCase {
    key: ENV_HTTP_READ_TIMEOUT_MS,
    value: Some("  2500  "),
    expected: Duration::from_millis(2500),
    expected_warning_error: None,
})]
fn duration_from_env_handles_input(
    empty_duration_warnings: EmptyDurationWarnings,
    #[case] case: DurationCase,
) {
    let entries = case.value.map_or_else(Vec::new, |configured_value| {
        vec![(case.key, configured_value)]
    });
    let env = fixture_env(&entries);

    let duration = duration_from_env(&env, case.key, Duration::from_secs(3));

    assert_eq!(duration, case.expected);
    let warnings = empty_duration_warnings.take();
    if let Some(expected_error) = case.expected_warning_error {
        assert_eq!(warnings.len(), 1);
        let warning = warnings.first().map_or("", String::as_str);
        assert!(
            warning.contains(case.key),
            "warning should mention the variable name"
        );
        assert!(
            warning.contains(expected_error),
            "warning should name the bounded parse failure, got {warning}"
        );
        // The value is caller-controlled, so it must never reach the log.
        if let Some(configured_value) = case.value.filter(|value| !value.is_empty()) {
            assert!(
                !warning.contains(configured_value),
                "warning must redact the offending value, got {warning}"
            );
        }
    } else {
        assert!(
            warnings.is_empty(),
            "valid or missing values should not warn"
        );
    }
}

#[test]
fn accept_connection_respects_accept_timeout() -> anyhow::Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    listener.set_nonblocking(true)?;

    let accept_timeout = Duration::from_millis(20);
    let poll_interval = Duration::from_millis(200);
    let start = Instant::now();
    let deadline = start + accept_timeout;

    let result = panic::catch_unwind(|| {
        drop(accept_connection(
            &listener,
            deadline,
            poll_interval,
            accept_timeout,
        ));
    });
    let Err(panic_payload) = result else {
        anyhow::bail!("accept_connection should panic when no client connects");
    };

    let elapsed = start.elapsed();
    anyhow::ensure!(
        elapsed >= accept_timeout,
        "panic should not occur before the accept timeout (elapsed {elapsed:?}, timeout {accept_timeout:?})",
    );
    anyhow::ensure!(
        elapsed <= accept_timeout + poll_interval + Duration::from_millis(500),
        "panic overshot accept timeout tolerance: elapsed={elapsed:?}, accept_timeout={accept_timeout:?}, poll_interval={poll_interval:?}",
    );

    let panic_ref = panic_payload.as_ref();
    let panic_text = panic_ref
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            panic_ref
                .downcast_ref::<&'static str>()
                .map(std::string::ToString::to_string)
        })
        .unwrap_or_else(|| format!("{panic_payload:?}"));
    anyhow::ensure!(
        panic_text.contains(&format!("accept_timeout={accept_timeout:?}")),
        "panic message should embed the accept timeout: {panic_text}",
    );
    anyhow::ensure!(
        panic_text.contains(&format!("poll_interval={poll_interval:?}")),
        "panic message should embed the poll interval: {panic_text}",
    );
    Ok(())
}
