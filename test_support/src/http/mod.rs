//! Lightweight HTTP fixtures for tests.
//!
//! Provides helpers for spawning simple HTTP servers that respond with a fixed
//! body. The listener is configured in non-blocking mode and guarded by a
//! deadline so hung clients cannot stall the test suite.

use mockable::{DefaultEnv, Env};
use std::{
    fmt, io,
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

mod accept;
mod request;
mod response;
mod server;

use self::accept::{AcceptWait, accept_connection};
pub use self::request::RequestLog;
pub use self::response::HttpResponse;
use self::server::{FixtureLedger, run_http_server};

/// Override for the timeout in milliseconds within which a client must connect.
pub(crate) const ENV_HTTP_ACCEPT_TIMEOUT_MS: &str = "NETSUKE_TEST_HTTP_ACCEPT_TIMEOUT_MS";
/// Override for the timeout in milliseconds within which the request must arrive.
pub(crate) const ENV_HTTP_READ_TIMEOUT_MS: &str = "NETSUKE_TEST_HTTP_READ_TIMEOUT_MS";
/// Override for the polling interval in milliseconds used while waiting.
pub(crate) const ENV_HTTP_POLL_INTERVAL_MS: &str = "NETSUKE_TEST_HTTP_POLL_INTERVAL_MS";

#[cfg(test)]
use std::{cell::RefCell, thread_local};

#[cfg(test)]
thread_local! {
    static DURATION_WARNINGS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

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

/// Spawn an HTTP server using `config`, emitting responses in sequence.
///
/// Returns the bound URL, the shared request count, the request log, and the
/// server handle. The public wrappers above reshape this tuple for their
/// callers, so every fixture shares one server implementation.
fn spawn_fixture_server(
    responses: impl IntoIterator<Item = HttpResponse>,
    config: HttpServerConfig,
) -> io::Result<(String, Arc<AtomicUsize>, RequestLog, HttpServer)> {
    let response_sequence = responses.into_iter().collect::<Vec<_>>();
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

/// Read `var` as whole milliseconds, falling back to `default` when unset or
/// unparsable.
fn duration_from_env(env: &impl Env, var: &str, default: Duration) -> Duration {
    env.raw(var).map_or(default, |value| {
        let trimmed = value.trim();
        match trimmed.parse::<u64>() {
            Ok(ms) => Duration::from_millis(ms),
            Err(err) => {
                log_duration_parse_error(var, trimmed.len(), &err);
                default
            }
        }
    })
}

/// Report an unparsable duration override without echoing its value.
///
/// The value is redacted: an environment variable's contents are outside this
/// crate's control, and logging them verbatim would put whatever the caller
/// exported into the log. `err` already names the bounded parse failure, and
/// `value_len` distinguishes an empty override from a malformed one, which is
/// all the diagnosis this fixture needs.
fn log_duration_parse_error(var: &str, value_len: usize, err: &dyn fmt::Display) {
    #[cfg(test)]
    {
        record_duration_warning(format!(
            "ignoring invalid {var}: {err} (value redacted, {value_len} bytes)"
        ));
    }

    #[cfg(not(test))]
    {
        tracing::warn!(
            variable = var,
            value_len,
            error = %err,
            "ignoring invalid fixture duration"
        );
    }
}

#[cfg(test)]
fn record_duration_warning(message: String) {
    DURATION_WARNINGS.with(|warnings| warnings.borrow_mut().push(message));
}

#[cfg(test)]
fn take_duration_warnings() -> Vec<String> {
    DURATION_WARNINGS.with(|warnings| warnings.borrow_mut().drain(..).collect())
}

#[cfg(test)]
mod config_tests;
#[cfg(test)]
mod tests;
