//! Lightweight HTTP fixtures for tests.
//!
//! Provides helpers for spawning simple HTTP servers that respond with a fixed
//! body. The listener is configured in non-blocking mode and guarded by a
//! deadline so hung clients cannot stall the test suite.

use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};

/// Spawn a single-use HTTP server that returns `body` for the first request.
///
/// The server listens on `127.0.0.1` and responds with a `200 OK` containing
/// the provided body. The listener is polled in non-blocking mode until a
/// client connects or a short deadline expires.
pub fn spawn_http_server(body: impl Into<String>) -> (String, thread::JoinHandle<()>) {
    let body = body.into();
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind HTTP listener");
    listener
        .set_nonblocking(true)
        .expect("set listener non-blocking");
    let addr = listener.local_addr().expect("local addr");
    let url = format!("http://{addr}");
    let handle = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buf = [0u8; 512];
                    let bytes_read = stream.read(&mut buf).unwrap_or(0);
                    if bytes_read > 0 {
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = stream.write_all(response.as_bytes());
                    }
                    break;
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    let timed_out = Instant::now() >= deadline;
                    assert!(!timed_out, "timed out waiting for fetch test connection");
                    thread::sleep(Duration::from_millis(10));
                }
                Err(err) => panic!("failed to accept connection: {err}"),
            }
        }
    });
    (url, handle)
}
