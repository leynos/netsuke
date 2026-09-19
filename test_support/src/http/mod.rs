//! Lightweight HTTP fixtures for tests.
//!
//! Provides helpers for spawning simple HTTP servers that respond with a fixed
//! body. The listener is configured in non-blocking mode and guarded by a
//! deadline so hung clients cannot stall the test suite.

use mockable::{DefaultEnv, Env};
use std::{
    io,
    net::{SocketAddr, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

mod accept;
mod env;
mod request;
mod response;
mod server;
mod spawn;

use self::accept::{AcceptWait, accept_connection};
use self::env::duration_from_env;
pub use self::request::RequestLog;
pub use self::response::{HttpResponse, RawHttpResponse};
use self::spawn::{spawn_fixture_server, spawn_raw_fixture_server};

/// Override for the timeout in milliseconds within which a client must connect.
pub(crate) const ENV_HTTP_ACCEPT_TIMEOUT_MS: &str = "NETSUKE_TEST_HTTP_ACCEPT_TIMEOUT_MS";
/// Override for the timeout in milliseconds within which the request must arrive.
pub(crate) const ENV_HTTP_READ_TIMEOUT_MS: &str = "NETSUKE_TEST_HTTP_READ_TIMEOUT_MS";
/// Override for the polling interval in milliseconds used while waiting.
pub(crate) const ENV_HTTP_POLL_INTERVAL_MS: &str = "NETSUKE_TEST_HTTP_POLL_INTERVAL_MS";

/// Configuration for HTTP fixtures, including timeouts used during polling.
#[derive(Debug, Clone)]
pub struct HttpServerConfig {
    /// Deadline for a client to connect.
    accept_timeout: Duration,
    /// Deadline for the request to finish arriving.
    read_timeout: Duration,
    /// Interval between readiness polls.
    poll_interval: Duration,
    /// Whether the accept loop waits for a client without a deadline.
    ///
    /// Set by fixtures that expect no request: nothing but a shutdown ends
    /// their wait, so a deadline there would fail a slow machine rather than a
    /// wrong test.
    accept_without_deadline: bool,
}

impl HttpServerConfig {
    /// Load configuration from environment variables, falling back to defaults.
    ///
    /// The following environment variables are honoured when present:
    ///
    /// * `NETSUKE_TEST_HTTP_ACCEPT_TIMEOUT_MS` – deadline for accepting a
    ///   connection in milliseconds.
    /// * `NETSUKE_TEST_HTTP_READ_TIMEOUT_MS` – deadline for reading the request
    ///   body in milliseconds.
    /// * `NETSUKE_TEST_HTTP_POLL_INTERVAL_MS` – polling interval used when
    ///   waiting for readiness in milliseconds.
    ///
    /// Notes:
    /// Polling interval overrides are clamped to a minimum of 1 ms to avoid
    /// busy-spinning when the environment provides `0`.
    #[must_use]
    pub fn from_env() -> Self {
        Self::from_env_provider(&DefaultEnv)
    }

    /// Load the configuration from `env`, clamping the poll interval to 1 ms.
    fn from_env_provider(env: &impl Env) -> Self {
        let mut config = Self::default();
        config.accept_timeout =
            duration_from_env(env, ENV_HTTP_ACCEPT_TIMEOUT_MS, config.accept_timeout);
        config.read_timeout = duration_from_env(env, ENV_HTTP_READ_TIMEOUT_MS, config.read_timeout);
        // Prevent busy-spin when overrides specify a zero-millisecond poll
        // interval. Tests only need millisecond precision, so clamp to at
        // least 1 ms.
        config.poll_interval =
            duration_from_env(env, ENV_HTTP_POLL_INTERVAL_MS, config.poll_interval)
                .max(Duration::from_millis(1));
        config
    }

    /// Return the instant by which a client must connect.
    fn accept_deadline(&self) -> Instant {
        Instant::now() + self.accept_timeout
    }

    /// Return a copy of this configuration that accepts without a deadline.
    #[must_use]
    const fn accepting_until_shutdown(mut self) -> Self {
        self.accept_without_deadline = true;
        self
    }

    /// Return what the accept loop should wait for.
    ///
    /// An unbounded wait is given `shutdown` so the loop can stop on the
    /// signal alone, without depending on the wake-up connection a join sends.
    fn accept_wait<'a>(&self, shutdown: &'a AtomicBool) -> AcceptWait<'a> {
        if self.accept_without_deadline {
            AcceptWait::UntilShutdown(shutdown)
        } else {
            AcceptWait::Until(self.accept_deadline())
        }
    }

    /// Return the instant by which the request must be read.
    fn read_deadline(&self) -> Instant {
        Instant::now() + self.read_timeout
    }
}

impl Default for HttpServerConfig {
    fn default() -> Self {
        Self {
            accept_timeout: Duration::from_secs(10),
            read_timeout: Duration::from_secs(5),
            poll_interval: Duration::from_millis(10),
            accept_without_deadline: false,
        }
    }
}

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

/// Spawn an HTTP server that emits each raw response in sequence.
///
/// # Raw responses
///
/// The bytes in `responses` are written to the client verbatim, so a caller
/// can emit a status line no HTTP client should accept. This exists because
/// the structured fixtures cannot: [`HttpResponse::new`] takes a status code,
/// so a response whose status line is malformed is not expressible at all,
/// and a test about a client's failure to parse one has to own the bytes.
///
/// The returned URL carries `redirect-user:redirect-secret` credentials, so a
/// test can drive a credentialed fetch — whose URL redaction is asserted
/// elsewhere — against malformed bytes rather than against a live host.
///
/// Every raw response is sent over the same fixture machinery as a structured
/// one: the same bounded accept, the same bounded request read, the same
/// accounting and shutdown. The request is drained before the bytes go out,
/// and the write half is then shut down rather than the socket closed, so the
/// response is delivered whole instead of racing a platform-dependent
/// transport abort. See `server::serve_raw_response` for the full reasoning.
///
/// # Errors
///
/// Returns an [`io::Error`] if the listener cannot be bound, switched to
/// non-blocking mode, queried for its local address, or if the fixture thread
/// fails to spawn. As with the structured fixtures, later I/O failures inside
/// the fixture thread panic it rather than returning.
pub fn spawn_http_server_raw_responses(
    responses: impl IntoIterator<Item = RawHttpResponse>,
) -> io::Result<(String, RequestLog, HttpServer)> {
    let (url, _requests, log, server) = spawn_raw_fixture_server(
        responses,
        HttpServerConfig::from_env().accepting_until_shutdown(),
    )?;
    Ok((url, log, server))
}

/// Spawn an HTTP server that emits one raw response.
///
/// The singular form of [`spawn_http_server_raw_responses`], for the common
/// case of a test that drives exactly one hop against malformed bytes.
///
/// # Errors
///
/// Propagates failures while starting the fixture server.
pub fn spawn_http_server_raw_response(
    response: RawHttpResponse,
) -> io::Result<(String, RequestLog, HttpServer)> {
    spawn_http_server_raw_responses([response])
}

#[cfg(test)]
mod config_tests;
#[cfg(test)]
mod tests;
