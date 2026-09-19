//! Lightweight HTTP fixtures for tests.
//!
//! Provides helpers for spawning simple HTTP servers that respond with a fixed
//! body. The listener is configured in non-blocking mode and guarded by a
//! deadline so hung clients cannot stall the test suite.

use std::{
    io,
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
};

mod accept;
mod config;
mod raw;
mod request;
mod response;
mod server;

pub use self::config::HttpServerConfig;
pub use self::raw::RawHttpResponse;
pub use self::request::RequestLog;
use self::response::FixtureResponse;
pub use self::response::HttpResponse;
use self::server::{FixtureLedger, run_http_server};

/// Join handle for a spawned HTTP fixture.
///
/// The handle joins the underlying thread when dropped to avoid leaking
/// background work if a test aborts early. Drop intentionally suppresses any
/// panic raised by the server thread so cleanup always completes; callers that
/// need to detect panics must invoke [`HttpServer::join`] and handle its
/// [`thread::Result`] instead of relying on the destructor.
#[derive(Debug)]
#[must_use]
pub struct HttpServer {
    /// The fixture thread's join handle.
    handle: Option<thread::JoinHandle<()>>,
    /// The bound listener address, used to wake a waiting accept loop.
    addr: SocketAddr,
    /// Set to stop the fixture accepting; the accept loop polls it.
    ///
    /// This, rather than the wake-up connection, is what ends the wait, so a
    /// fixture whose wake-up never arrives still shuts down.
    shutdown: Arc<AtomicBool>,
}

impl HttpServer {
    /// Join the server thread and propagate any panic.
    ///
    /// # Errors
    ///
    /// Returns the server thread's panic payload if the thread panicked.
    pub fn join(mut self) -> thread::Result<()> {
        self.shutdown_listener();
        self.handle
            .take()
            .map_or_else(|| Ok(()), std::thread::JoinHandle::join)
    }

    /// Signal the fixture to stop accepting, and wake a waiting accept loop.
    ///
    /// The flag is the shutdown condition, so the wait ends whether or not the
    /// connection below arrives; the connect only shortens it, and its outcome
    /// is deliberately ignored.
    fn shutdown_listener(&self) {
        self.shutdown.store(true, Ordering::Release);
        // Wake the accept loop promptly. A failed connect is harmless: the flag
        // set above, not this connection, is what ends the wait.
        drop(TcpStream::connect(self.addr));
    }
}

impl Drop for HttpServer {
    fn drop(&mut self) {
        self.shutdown_listener();
        if let Some(handle) = self.handle.take() {
            drop(handle.join());
        }
    }
}

/// Spawn a single-use HTTP server that returns `body` for the first request.
///
/// The server listens on `127.0.0.1` and responds with a `200 OK` containing
/// the provided body. The listener is polled in non-blocking mode until a
/// client connects or a short deadline expires.
///
/// # Configuration
/// Timeouts are loaded from the environment via
/// [`HttpServerConfig::from_env`]:
/// - `NETSUKE_TEST_HTTP_ACCEPT_TIMEOUT_MS`
/// - `NETSUKE_TEST_HTTP_READ_TIMEOUT_MS`
/// - `NETSUKE_TEST_HTTP_POLL_INTERVAL_MS`
///   (values below 1 ms are clamped to 1 ms to avoid busy-spinning)
///
/// # Errors
/// Returns an [`io::Error`] if the listener cannot be bound, switched to
/// non-blocking mode, queried for its local address, or if the fixture thread
/// fails to spawn.
pub fn spawn_http_server(body: impl Into<String>) -> io::Result<(String, HttpServer)> {
    spawn_http_server_with_config(body, HttpServerConfig::from_env())
}

/// Spawn a single-use HTTP server using the provided configuration.
///
/// # Errors
/// Propagates any [`io::Error`] encountered when binding the listener,
/// switching it to non-blocking mode, querying its local address, or spawning
/// the fixture thread. Subsequent operations may panic if unexpected I/O
/// conditions occur while handling the client connection.
pub fn spawn_http_server_with_config(
    response_body: impl Into<String>,
    config: HttpServerConfig,
) -> io::Result<(String, HttpServer)> {
    let (url, _requests, _log, server) =
        spawn_fixture_server([HttpResponse::new(200, response_body)], config)?;
    Ok((url, server))
}

/// Spawn an HTTP server that emits each response in sequence and counts requests.
///
/// # Errors
///
/// Propagates failures while starting the fixture server.
pub fn spawn_http_server_responses(
    responses: impl IntoIterator<Item = HttpResponse>,
) -> io::Result<(String, Arc<AtomicUsize>, HttpServer)> {
    let (url, requests, _log, server) =
        spawn_fixture_server(responses, HttpServerConfig::from_env())?;
    Ok((url, requests, server))
}

/// Spawn an HTTP server that emits each response in sequence and records the
/// request line of every request it answers.
///
/// The request log lets a test assert the method and target a client used at
/// each hop of a redirect chain, which a request count alone cannot show.
///
/// # Errors
///
/// Propagates failures while starting the fixture server.
pub fn spawn_http_server_recording(
    responses: impl IntoIterator<Item = HttpResponse>,
) -> io::Result<(String, RequestLog, HttpServer)> {
    let (url, _requests, log, server) =
        spawn_fixture_server(responses, HttpServerConfig::from_env())?;
    Ok((url, log, server))
}

/// Spawn a fixture for a hop or target that must receive no request.
///
/// The fixture answers and records any request it does receive, so a test can
/// assert the log stayed empty, but it waits for a connection without the
/// accept deadline the other fixtures use. Nothing but the shutdown signal
/// [`HttpServer::join`] raises ends that wait, so a deadline would fail a slow
/// machine rather than a wrong test.
///
/// # Errors
///
/// Propagates failures while starting the fixture server.
pub fn spawn_http_server_expecting_no_requests(
    response: HttpResponse,
) -> io::Result<(String, RequestLog, HttpServer)> {
    let (url, _requests, log, server) = spawn_fixture_server(
        [response],
        HttpServerConfig::from_env().accepting_until_shutdown(),
    )?;
    Ok((url, log, server))
}

/// Spawn a single-use HTTP server that returns raw bytes for the first request.
///
/// The payload is emitted verbatim and the response is completed with the same
/// write-side shutdown every other fixture uses, so a test of a client's parse
/// failures sees the bytes it wrote rather than a connection the fixture's own
/// teardown aborted. Use this instead of standing a bare listener in a test: a
/// listener that closes without that shutdown races the client, and on Windows
/// the client then reports an aborted connection in place of the fault the
/// payload was written to provoke.
///
/// The fixture still reads a complete request before it answers, so the client's
/// request bytes are consumed rather than left to force a reset. The request
/// counter is returned so a case can assert the malformed response was actually
/// solicited.
///
/// # Configuration
/// Timeouts are loaded from the environment via
/// [`HttpServerConfig::from_env`], as for [`spawn_http_server`].
///
/// # Errors
/// Propagates failures while starting the fixture server.
pub fn spawn_raw_http_server(
    response: RawHttpResponse,
) -> io::Result<(String, Arc<AtomicUsize>, HttpServer)> {
    let (url, requests, _log, server) =
        spawn_fixture_server([response], HttpServerConfig::from_env())?;
    Ok((url, requests, server))
}

/// Spawn an HTTP server using `config`, emitting responses in sequence.
///
/// Returns the bound URL, the shared request count, the request log, and the
/// server handle. The public wrappers above reshape this tuple for their
/// callers, so every fixture shares one server implementation, and the responses
/// are stored behind [`FixtureResponse`] so a wrapper may configure either a
/// checked rendering or a raw payload without a second implementation.
fn spawn_fixture_server(
    responses: impl IntoIterator<Item = impl FixtureResponse + 'static>,
    config: HttpServerConfig,
) -> io::Result<(String, Arc<AtomicUsize>, RequestLog, HttpServer)> {
    let response_sequence = responses
        .into_iter()
        .map(|response| Box::new(response) as Box<dyn FixtureResponse + Send>)
        .collect::<Vec<_>>();
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    listener.set_nonblocking(true)?;
    let addr = listener.local_addr()?;
    let url = format!("http://{addr}");
    let requests = Arc::new(AtomicUsize::new(0));
    let server_requests = Arc::clone(&requests);
    let log = RequestLog::default();
    let server_log = log.clone();
    let shutdown = Arc::new(AtomicBool::new(false));
    let server_shutdown = Arc::clone(&shutdown);
    let handle = thread::Builder::new()
        .name("netsuke-http-fixture".into())
        .spawn(move || {
            run_http_server(
                &listener,
                &response_sequence,
                &config,
                &FixtureLedger::new(&server_requests, &server_log, &server_shutdown),
            );
        })?;
    Ok((
        url,
        requests,
        log,
        HttpServer {
            handle: Some(handle),
            addr,
            shutdown,
        },
    ))
}

#[cfg(test)]
#[path = "raw_tests.rs"]
mod raw_tests;
#[cfg(test)]
mod tests;
