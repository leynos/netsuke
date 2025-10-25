//! Lightweight HTTP fixtures for tests.
//!
//! Provides helpers for spawning simple HTTP servers that respond with a fixed
//! body. The listener is configured in non-blocking mode and guarded by a
//! deadline so hung clients cannot stall the test suite.

use std::{
    env, fmt,
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    thread,
    time::{Duration, Instant},
};

pub(crate) const ENV_HTTP_ACCEPT_TIMEOUT_MS: &str = "NETSUKE_TEST_HTTP_ACCEPT_TIMEOUT_MS";
pub(crate) const ENV_HTTP_READ_TIMEOUT_MS: &str = "NETSUKE_TEST_HTTP_READ_TIMEOUT_MS";
pub(crate) const ENV_HTTP_POLL_INTERVAL_MS: &str = "NETSUKE_TEST_HTTP_POLL_INTERVAL_MS";

#[cfg(test)]
use std::{cell::RefCell, thread_local};

#[cfg(test)]
thread_local! {
    static DURATION_WARNINGS: RefCell<Vec<String>> = RefCell::new(Vec::new());
}

/// Configuration for HTTP fixtures, including timeouts used during polling.
#[derive(Debug, Clone)]
pub struct HttpServerConfig {
    accept_timeout: Duration,
    read_timeout: Duration,
    poll_interval: Duration,
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
    pub fn from_env() -> Self {
        let mut config = Self::default();
        config.accept_timeout =
            duration_from_env(ENV_HTTP_ACCEPT_TIMEOUT_MS, config.accept_timeout);
        config.read_timeout = duration_from_env(ENV_HTTP_READ_TIMEOUT_MS, config.read_timeout);
        // Prevent busy-spin when overrides specify a zero-millisecond poll
        // interval. Tests only need millisecond precision, so clamp to at
        // least 1 ms.
        config.poll_interval = duration_from_env(ENV_HTTP_POLL_INTERVAL_MS, config.poll_interval)
            .max(Duration::from_millis(1));
        config
    }

    fn accept_deadline(&self) -> Instant {
        Instant::now() + self.accept_timeout
    }

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
    handle: Option<thread::JoinHandle<()>>,
    addr: SocketAddr,
}

impl HttpServer {
    /// Join the server thread and propagate any panic.
    pub fn join(mut self) -> thread::Result<()> {
        self.shutdown_listener();
        self.handle.take().expect("server already joined").join()
    }

    fn shutdown_listener(&self) {
        // Connect to unblock the accept loop; the outcome is irrelevant.
        let _ = TcpStream::connect(self.addr);
    }
}

impl Drop for HttpServer {
    fn drop(&mut self) {
        self.shutdown_listener();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
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
/// # Panics
/// See [`spawn_http_server_with_config`] for potential panic conditions.
pub fn spawn_http_server(body: impl Into<String>) -> (String, HttpServer) {
    spawn_http_server_with_config(body, HttpServerConfig::from_env())
}

/// Spawn a single-use HTTP server using the provided configuration.
///
/// # Panics
///
/// Panics if:
/// - binding the listener fails (for example, when ephemeral ports are
///   exhausted).
/// - switching the listener or accepted stream to non-blocking mode fails.
/// - accepting a connection or reading from the socket yields unexpected I/O
///   errors.
pub fn spawn_http_server_with_config(
    body: impl Into<String>,
    config: HttpServerConfig,
) -> (String, HttpServer) {
    let body = body.into();
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind HTTP listener");
    listener
        .set_nonblocking(true)
        .expect("set listener non-blocking");
    let addr = listener.local_addr().expect("local addr");
    let url = format!("http://{addr}");
    let handle = thread::Builder::new()
        .name("netsuke-http-fixture".into())
        .spawn(move || run_http_server(listener, body, config))
        .expect("spawn http fixture thread");
    (
        url,
        HttpServer {
            handle: Some(handle),
            addr,
        },
    )
}

fn run_http_server(listener: TcpListener, body: String, config: HttpServerConfig) {
    let mut stream = accept_connection(
        &listener,
        config.accept_deadline(),
        config.poll_interval,
        config.accept_timeout,
    );
    stream
        .set_nonblocking(true)
        .expect("set stream non-blocking");
    let bytes_read = read_request(&mut stream, config.read_deadline(), config.poll_interval);
    if bytes_read > 0 {
        write_response(&mut stream, &body);
    }
}

fn is_past_deadline(deadline: Instant) -> bool {
    Instant::now() >= deadline
}

fn should_retry_accept(
    err: &io::Error,
    deadline: Instant,
    poll_interval: Duration,
    accept_timeout: Duration,
) -> bool {
    if is_past_deadline(deadline) {
        panic!(
            "timed out waiting for fetch test connection (accept_timeout={:?}, poll_interval={:?})",
            accept_timeout, poll_interval
        );
    }
    // Treat transient readiness states (EAGAIN/EWOULDBLOCK) and EINTR as retryable.
    matches!(
        err.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
    )
}

fn remaining_until_deadline(deadline: Instant) -> Duration {
    let now = Instant::now();
    if deadline > now {
        deadline - now
    } else {
        Duration::from_millis(0)
    }
}

fn accept_connection(
    listener: &TcpListener,
    deadline: Instant,
    poll_interval: Duration,
    accept_timeout: Duration,
) -> TcpStream {
    loop {
        match listener.accept() {
            Ok((stream, _)) => return stream,
            Err(err) if should_retry_accept(&err, deadline, poll_interval, accept_timeout) => {
                let remaining = remaining_until_deadline(deadline);
                thread::sleep(remaining.min(poll_interval));
            }
            Err(err) => panic!("failed to accept connection: {err}"),
        }
    }
}

fn try_read(stream: &mut TcpStream) -> Option<usize> {
    let mut buf = [0u8; 512];
    match stream.read(&mut buf) {
        Ok(0) => Some(0),
        Ok(n) => Some(n),
        Err(err) if err.kind() == io::ErrorKind::WouldBlock => None,
        Err(err) => panic!("failed to read request: {err}"),
    }
}

fn read_request(stream: &mut TcpStream, deadline: Instant, poll_interval: Duration) -> usize {
    loop {
        match try_read(stream) {
            Some(bytes_read) => return bytes_read,
            None => {
                if Instant::now() >= deadline {
                    return 0;
                }
                thread::sleep(poll_interval);
            }
        }
    }
}

fn write_response(stream: &mut TcpStream, body: &str) {
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
}

fn duration_from_env(var: &str, default: Duration) -> Duration {
    match env::var(var) {
        Ok(value) => {
            let trimmed = value.trim();
            match trimmed.parse::<u64>() {
                Ok(ms) => Duration::from_millis(ms),
                Err(err) => {
                    log_duration_parse_error(var, value.as_str(), &err);
                    default
                }
            }
        }
        Err(_) => default,
    }
}

fn log_duration_parse_error(var: &str, value: &str, err: &dyn fmt::Display) {
    #[cfg(test)]
    {
        record_duration_warning(format!("ignoring invalid {var}='{value}': {err}"));
    }

    #[cfg(not(test))]
    {
        eprintln!("netsuke: ignoring invalid {var} value '{value}': {err}");
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
mod tests {
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
        let panic_payload =
            result.expect_err("accept_connection should panic when no client connects");

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
}
