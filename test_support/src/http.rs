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

#[cfg(test)]
use std::sync::{Mutex, OnceLock};

#[cfg(test)]
static DURATION_WARNINGS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

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
    pub fn from_env() -> Self {
        Self {
            accept_timeout: duration_from_env(
                "NETSUKE_TEST_HTTP_ACCEPT_TIMEOUT_MS",
                Duration::from_secs(10),
            ),
            read_timeout: duration_from_env(
                "NETSUKE_TEST_HTTP_READ_TIMEOUT_MS",
                Duration::from_secs(5),
            ),
            poll_interval: duration_from_env(
                "NETSUKE_TEST_HTTP_POLL_INTERVAL_MS",
                Duration::from_millis(10),
            ),
        }
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
pub fn spawn_http_server(body: impl Into<String>) -> (String, HttpServer) {
    spawn_http_server_with_config(body, HttpServerConfig::from_env())
}

/// Spawn a single-use HTTP server using the provided configuration.
#[allow(clippy::missing_panics_doc)]
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
    let handle = thread::spawn(move || run_http_server(listener, body, config));
    (
        url,
        HttpServer {
            handle: Some(handle),
            addr,
        },
    )
}

fn run_http_server(listener: TcpListener, body: String, config: HttpServerConfig) {
    let mut stream = accept_connection(&listener, config.accept_deadline(), config.poll_interval);
    stream
        .set_nonblocking(true)
        .expect("set stream non-blocking");
    let bytes_read = read_request(&mut stream, config.read_deadline(), config.poll_interval);
    if bytes_read > 0 {
        write_response(&mut stream, &body);
    }
}

fn accept_connection(
    listener: &TcpListener,
    deadline: Instant,
    poll_interval: Duration,
) -> TcpStream {
    loop {
        match listener.accept() {
            Ok((stream, _)) => return stream,
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                assert!(
                    Instant::now() < deadline,
                    "timed out waiting for fetch test connection"
                );
                thread::sleep(poll_interval);
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
        Ok(value) => value
            .parse::<u64>()
            .map(Duration::from_millis)
            .unwrap_or_else(|err| {
                log_duration_parse_error(var, &value, &err);
                default
            }),
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
    let warnings = DURATION_WARNINGS.get_or_init(|| Mutex::new(Vec::new()));
    warnings
        .lock()
        .expect("duration warnings lock")
        .push(message);
}

#[cfg(test)]
fn take_duration_warnings() -> Vec<String> {
    let warnings = DURATION_WARNINGS.get_or_init(|| Mutex::new(Vec::new()));
    std::mem::take(&mut *warnings.lock().expect("duration warnings lock"))
}

#[cfg(test)]
fn clear_duration_warnings() {
    let _ = take_duration_warnings();
}

#[cfg(test)]
mod tests {
    use super::{
        HttpServerConfig, clear_duration_warnings, duration_from_env, take_duration_warnings,
    };

    use crate::{EnvVarGuard, env_lock::EnvLock};
    use std::time::Duration;

    #[test]
    fn from_env_applies_overrides() {
        let _lock = EnvLock::acquire();
        clear_duration_warnings();
        let accept = EnvVarGuard::set("NETSUKE_TEST_HTTP_ACCEPT_TIMEOUT_MS", "1500");
        let read = EnvVarGuard::set("NETSUKE_TEST_HTTP_READ_TIMEOUT_MS", "750");
        let poll = EnvVarGuard::set("NETSUKE_TEST_HTTP_POLL_INTERVAL_MS", "25");

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
    fn duration_from_env_returns_default_for_missing() {
        let _lock = EnvLock::acquire();
        clear_duration_warnings();
        let guard = EnvVarGuard::remove("NETSUKE_TEST_HTTP_ACCEPT_TIMEOUT_MS");
        let duration = duration_from_env(
            "NETSUKE_TEST_HTTP_ACCEPT_TIMEOUT_MS",
            Duration::from_secs(3),
        );
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
        clear_duration_warnings();
        let guard = EnvVarGuard::set("NETSUKE_TEST_HTTP_ACCEPT_TIMEOUT_MS", "not-a-number");
        let duration = duration_from_env(
            "NETSUKE_TEST_HTTP_ACCEPT_TIMEOUT_MS",
            Duration::from_secs(3),
        );
        assert_eq!(duration, Duration::from_secs(3));
        let warnings = take_duration_warnings();
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0].contains("NETSUKE_TEST_HTTP_ACCEPT_TIMEOUT_MS"),
            "warning should mention the variable name"
        );
        assert!(
            warnings[0].contains("not-a-number"),
            "warning should include the invalid value"
        );
        drop(guard);
    }
}
