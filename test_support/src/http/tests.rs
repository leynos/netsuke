//! Unit tests for the HTTP fixture implementation in the parent module.
//!
//! These tests exercise timeout configuration, connection acceptance, and
//! warning capture without exposing test-only helpers through `http`'s public
//! interface.

use super::{
    ENV_HTTP_ACCEPT_TIMEOUT_MS, ENV_HTTP_POLL_INTERVAL_MS, ENV_HTTP_READ_TIMEOUT_MS,
    HttpServerConfig, accept_connection, duration_from_env, take_duration_warnings,
};
use super::{HttpResponse, response::render_response};

use mockable::MockEnv;
use rstest::{fixture, rstest};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    panic,
    time::{Duration, Instant},
};

#[test]
fn response_rendering_preserves_status_headers_and_body() {
    let response = HttpResponse::new(302, "next")
        .with_header("Location", "/redirected")
        .with_header("X-Test", "fixture");

    assert_eq!(
        render_response(&response),
        "HTTP/1.1 302 Found\r\nLocation: /redirected\r\nX-Test: fixture\r\nContent-Length: 4\r\nConnection: close\r\n\r\nnext"
    );
}

#[test]
fn response_server_counts_each_client_request() -> anyhow::Result<()> {
    let (url, requests, server) = super::spawn_http_server_responses([
        HttpResponse::new(302, "").with_header("Location", "/next"),
        HttpResponse::new(200, "done"),
    ])?;

    send_request(&url)?;
    send_request(&url)?;
    server
        .join()
        .map_err(|err| anyhow::anyhow!("fixture server panicked: {err:?}"))?;

    anyhow::ensure!(
        requests.load(std::sync::atomic::Ordering::Relaxed) == 2,
        "fixture should count both client requests",
    );
    Ok(())
}

/// Send one minimal HTTP request to the fixture at `url`.
fn send_request(url: &str) -> anyhow::Result<()> {
    let address = url
        .strip_prefix("http://")
        .ok_or_else(|| anyhow::anyhow!("fixture URL must use HTTP: {url}"))?;
    let mut stream = TcpStream::connect(address)?;
    stream.write_all(b"GET / HTTP/1.1\r\nHost: fixture\r\nConnection: close\r\n\r\n")?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    anyhow::ensure!(
        response.starts_with("HTTP/1.1 "),
        "fixture response should begin with an HTTP status line",
    );
    Ok(())
}

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
    /// Byte length the warning should report for the redacted value.
    ///
    /// Measured after trimming, matching the call site. This is the one piece of
    /// shape the redaction still surfaces, so pinning it stops the length going
    /// missing — or turning back into the value — unnoticed.
    expected_warning_len: Option<usize>,
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
    expected_warning_len: None,
})]
#[case::invalid(DurationCase {
    key: ENV_HTTP_ACCEPT_TIMEOUT_MS,
    value: Some("not-a-number"),
    expected: Duration::from_secs(3),
    expected_warning_error: Some("invalid digit"),
    expected_warning_len: Some("not-a-number".len()),
})]
#[case::empty(DurationCase {
    key: ENV_HTTP_ACCEPT_TIMEOUT_MS,
    value: Some(""),
    expected: Duration::from_secs(3),
    expected_warning_error: Some("cannot parse integer from empty string"),
    expected_warning_len: Some(0),
})]
#[case::whitespace_padded(DurationCase {
    key: ENV_HTTP_READ_TIMEOUT_MS,
    value: Some("  2500  "),
    expected: Duration::from_millis(2500),
    expected_warning_error: None,
    expected_warning_len: None,
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
        if let Some(expected_len) = case.expected_warning_len {
            assert!(
                warning.contains(&format!("{expected_len} bytes")),
                "warning should report the redacted value's byte length, got {warning}"
            );
        }
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

proptest::proptest! {
    /// The redaction must hold for any value a caller might export, not just
    /// the table's sentinels.
    ///
    /// Asserting the whole rendered warning against a message rebuilt from
    /// bounded parts is stronger than a "does not contain the value" check: it
    /// leaves the value nowhere to hide, and it cannot be fooled by a generated
    /// value that happens to be a substring of the template itself — `bytes`,
    /// for instance, would satisfy a naive `!contains` assertion.
    #[test]
    fn invalid_duration_warnings_are_composed_only_of_bounded_parts(
        raw in r"[^0-9\s][^\s]{0,24}",
    ) {
        let trimmed = raw.trim();
        // A leading `+` still parses as u64, so filter rather than assume the
        // strategy only yields rejects.
        proptest::prop_assume!(trimmed.parse::<u64>().is_err());
        let parse_error = trimmed
            .parse::<u64>()
            .expect_err("guarded by the assumption above");

        // Drain any residue so this case observes only the warning it caused.
        drop(take_duration_warnings());
        let env = fixture_env(&[(ENV_HTTP_ACCEPT_TIMEOUT_MS, raw.as_str())]);
        let default = Duration::from_secs(3);

        let duration = duration_from_env(&env, ENV_HTTP_ACCEPT_TIMEOUT_MS, default);

        proptest::prop_assert_eq!(duration, default);
        let warnings = take_duration_warnings();
        proptest::prop_assert_eq!(warnings.len(), 1);
        // The variable name is a crate constant, the parse error is one of
        // `ParseIntError`'s fixed messages, and the length is a number: an exact
        // match therefore proves no caller-supplied byte reached the log.
        let expected = format!(
            "ignoring invalid {ENV_HTTP_ACCEPT_TIMEOUT_MS}: {parse_error} (value redacted, {} bytes)",
            trimmed.len()
        );
        proptest::prop_assert_eq!(warnings.first().cloned().unwrap_or_default(), expected);
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
