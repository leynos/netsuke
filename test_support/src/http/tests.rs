//! Unit tests for the HTTP fixture implementation in the parent module.
//!
//! These tests exercise connection acceptance and fixture response behaviour
//! without exposing test-only helpers through `http`'s public interface.
//! Timeout configuration and its warnings live in the sibling `config_tests`
//! module.

use super::{
    AcceptWait, HttpResponse, HttpServerConfig, RawHttpResponse, accept_connection,
    response::render_response,
};

use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    panic,
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
    let mut stream = connect_fixture(url)?;
    stream.write_all(b"GET / HTTP/1.1\r\nHost: fixture\r\nConnection: close\r\n\r\n")?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    anyhow::ensure!(
        response.starts_with("HTTP/1.1 "),
        "fixture response should begin with an HTTP status line",
    );
    Ok(())
}

/// Connect to the loopback address in a fixture `url`.
///
/// The raw fixture advertises a URL carrying credentials, which a client must
/// not send as part of the authority, so they are stripped here. The
/// structured fixture advertises none, and takes the same path.
fn connect_fixture(url: &str) -> anyhow::Result<TcpStream> {
    let authority = url
        .strip_prefix("http://")
        .ok_or_else(|| anyhow::anyhow!("fixture URL must use HTTP: {url}"))?;
    let address = authority
        .rsplit_once('@')
        .map_or(authority, |(_userinfo, host)| host);
    Ok(TcpStream::connect(address)?)
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
            AcceptWait::Until(deadline),
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

/// A raw response reaches the client byte for byte, framed by a half-close.
///
/// This is the fixture contract the redirect error tests rest on, and it is
/// asserted here rather than only through those tests because the failure it
/// guards against is silent: a fixture that wrote the bytes and then closed
/// the socket outright could still pass a client-side test on Linux, while
/// aborting the connection on Windows. Reading to EOF is what proves the
/// response was framed — the read terminates on the shutdown, not on a reset —
/// and the exact-byte comparison is what proves nothing was rewritten in
/// flight. The raw bytes are deliberately something no real server should
/// send, because that is the only input the fixture exists to serve.
#[test]
fn raw_response_fixture_delivers_the_exact_bytes_it_was_given() -> anyhow::Result<()> {
    let malformed = b"HTTP/1.1 banana OK\r\nContent-Length: 0\r\n\r\n";
    let (url, log, server) =
        super::spawn_http_server_raw_response(RawHttpResponse::new(malformed))?;
    anyhow::ensure!(
        url.contains("redirect-user:redirect-secret@"),
        "the raw fixture URL should carry credentials: {url}",
    );

    let received = send_request_and_read_to_eof(&url)?;
    server
        .join()
        .map_err(|err| anyhow::anyhow!("fixture server panicked: {err:?}"))?;

    anyhow::ensure!(
        received == malformed,
        "the fixture should deliver the bytes it was given, got {:?}",
        String::from_utf8_lossy(&received),
    );
    anyhow::ensure!(
        log.lines() == vec!["GET / HTTP/1.1".to_owned()],
        "the raw fixture should record the request it answered: {:?}",
        log.lines(),
    );
    Ok(())
}

/// Send one minimal HTTP request and read the response back to EOF.
///
/// Unlike [`send_request`], this returns the bytes rather than asserting a
/// status line, because the caller is testing responses that have none.
fn send_request_and_read_to_eof(url: &str) -> anyhow::Result<Vec<u8>> {
    let mut stream = connect_fixture(url)?;
    stream.write_all(b"GET / HTTP/1.1\r\nHost: fixture\r\nConnection: close\r\n\r\n")?;
    let mut received = Vec::new();
    stream.read_to_end(&mut received)?;
    Ok(received)
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
