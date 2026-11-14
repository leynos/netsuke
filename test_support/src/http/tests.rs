//! Unit tests for the HTTP fixture implementation in the parent module.
//!
//! These tests exercise connection acceptance and fixture response behaviour
//! without exposing test-only helpers through `http`'s public interface.
//! Timeout configuration and its warnings live in the sibling `config_tests`
//! module.

use super::{
    HttpResponse, HttpServerConfig,
    accept::{AcceptWait, accept_connection, nap_duration},
    response::render_response,
};

use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
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

/// The accept loop never sleeps past its deadline.
///
/// This replaces the elapsed-time bounds the panic test used to assert after
/// catching the panic: the responsiveness lives in the nap length, which can be
/// checked directly and without timing.
#[test]
fn nap_is_shortened_to_the_time_remaining() {
    let poll_interval = Duration::from_millis(200);

    let near = nap_duration(
        AcceptWait::Until(Instant::now() + Duration::from_millis(20)),
        poll_interval,
    );
    assert!(
        near <= Duration::from_millis(20),
        "a 20 ms deadline must not be overslept by a full poll interval, got {near:?}"
    );

    let far = nap_duration(
        AcceptWait::Until(Instant::now() + Duration::from_secs(60)),
        poll_interval,
    );
    assert_eq!(
        far, poll_interval,
        "a distant deadline polls at the interval"
    );

    let passed = nap_duration(AcceptWait::Until(Instant::now()), poll_interval);
    assert_eq!(passed, Duration::ZERO, "a passed deadline does not sleep");

    let unbounded = AtomicBool::new(false);
    assert_eq!(
        nap_duration(AcceptWait::UntilShutdown(&unbounded), poll_interval),
        poll_interval,
        "an unbounded wait polls at the interval"
    );
}

/// With no client, `accept_connection` panics once the deadline passes, and the
/// message names both the accept timeout and the poll interval.
///
/// The panic is asserted with `#[should_panic]` so the test does not depend on
/// unwinding. The panic is raised only once the deadline has passed, so a
/// missing or early panic still fails this test; how promptly it arrives is
/// covered by `nap_is_shortened_to_the_time_remaining`.
#[test]
#[should_panic(
    expected = "timed out waiting for fetch test connection (accept_timeout=20ms, poll_interval=200ms)"
)]
fn accept_connection_respects_accept_timeout() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind a loopback listener");
    listener
        .set_nonblocking(true)
        .expect("make the listener non-blocking");

    let accept_timeout = Duration::from_millis(20);
    let poll_interval = Duration::from_millis(200);
    let deadline = Instant::now() + accept_timeout;

    drop(accept_connection(
        &listener,
        AcceptWait::Until(deadline),
        poll_interval,
        accept_timeout,
    ));
}

/// A signalled shutdown must end an unbounded accept wait by itself.
///
/// The wake-up connection a join sends is best-effort, so the accept loop must
/// not depend on it: were that connect to fail, an unbounded wait whose only
/// exit was the connection would never end, and the join would hang rather
/// than fail. Arming the flag with no client ever connecting proves the loop
/// stops on the signal alone.
///
/// The wait is bounded by `recv_timeout` rather than by joining the probe
/// thread, so a regression fails this test instead of hanging the suite — the
/// very failure mode the assertion exists to catch.
#[test]
fn shutdown_signal_ends_the_accept_wait_without_a_wake_up_connection() -> anyhow::Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    listener.set_nonblocking(true)?;
    let shutdown = Arc::new(AtomicBool::new(false));
    let accept_shutdown = Arc::clone(&shutdown);
    let (ended_tx, ended_rx) = mpsc::channel();

    thread::Builder::new()
        .name("accept-wait-probe".into())
        .spawn(move || {
            let accepted = accept_connection(
                &listener,
                AcceptWait::UntilShutdown(&accept_shutdown),
                Duration::from_millis(5),
                Duration::from_secs(10),
            );
            // The receiver reports the outcome; a dropped one means this test
            // has already failed, so there is nobody left to tell.
            drop(ended_tx.send(accepted));
        })?;

    // Let the probe reach the loop, then shut it down without connecting.
    thread::sleep(Duration::from_millis(50));
    shutdown.store(true, Ordering::Release);

    let accepted = ended_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("a signalled shutdown must end the accept wait");
    anyhow::ensure!(
        accepted.is_none(),
        "a shut down fixture must report no connection, not a stream",
    );
    Ok(())
}

/// A fixture that expects no request must not fail merely for waiting.
///
/// The accept watchdog is a liveness guard for fixtures whose request the test
/// expects. Applying it to a fixture that expects none failed Windows CI: the
/// wait outlasted the accept timeout while the chain was still being driven, so
/// the fixture thread panicked before the test could join the server.
#[test]
fn expect_no_requests_fixture_waits_past_the_accept_timeout() -> anyhow::Result<()> {
    let accept_timeout = Duration::from_millis(20);
    let config = HttpServerConfig {
        accept_timeout,
        ..HttpServerConfig::default()
    }
    .accepting_until_shutdown();
    let (url, requests, log, server) =
        super::spawn_fixture_server([HttpResponse::new(200, "unexpected request")], config)?;
    anyhow::ensure!(
        url.starts_with("http://"),
        "fixture should expose a URL: {url}"
    );

    // Outwait the accept timeout several times over without connecting.
    thread::sleep(accept_timeout * 10);

    server
        .join()
        .map_err(|err| anyhow::anyhow!("fixture server panicked: {err:?}"))?;
    anyhow::ensure!(
        requests.load(std::sync::atomic::Ordering::Relaxed) == 0,
        "an uncontacted fixture should answer nothing",
    );
    anyhow::ensure!(
        log.is_empty(),
        "an uncontacted fixture should record nothing: {:?}",
        log.lines(),
    );
    Ok(())
}

/// The no-request fixture still records a request it receives.
///
/// Without this, an empty log would also hold for a fixture that never records
/// anything, leaving the assertions that rely on it vacuous.
#[test]
fn expect_no_requests_fixture_records_a_request_it_receives() -> anyhow::Result<()> {
    let (url, log, server) =
        super::spawn_http_server_expecting_no_requests(HttpResponse::new(200, "unexpected"))?;

    send_request(&url)?;
    server
        .join()
        .map_err(|err| anyhow::anyhow!("fixture server panicked: {err:?}"))?;

    anyhow::ensure!(
        log.lines() == vec!["GET / HTTP/1.1".to_owned()],
        "the fixture should record the request it answered: {:?}",
        log.lines(),
    );
    Ok(())
}
