//! Lightweight HTTP fixtures for tests.
//!
//! Provides helpers for spawning simple HTTP servers that respond with a fixed
//! body. The listener is configured in non-blocking mode and guarded by a
//! deadline so hung clients cannot stall the test suite.

use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    thread,
    time::{Duration, Instant},
};

/// Join handle for a spawned HTTP fixture.
///
/// The handle joins the underlying thread when dropped to avoid leaking
/// background work if a test aborts early. Call [`HttpServer::join`] to surface
/// any panic from the server thread explicitly.
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
    let body = body.into();
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind HTTP listener");
    listener
        .set_nonblocking(true)
        .expect("set listener non-blocking");
    let addr = listener.local_addr().expect("local addr");
    let url = format!("http://{addr}");
    let handle = thread::spawn(move || run_http_server(listener, body));
    (
        url,
        HttpServer {
            handle: Some(handle),
            addr,
        },
    )
}

fn run_http_server(listener: TcpListener, body: String) {
    let accept_deadline = Instant::now() + Duration::from_secs(2);
    let mut stream = accept_connection(&listener, accept_deadline);
    stream
        .set_nonblocking(true)
        .expect("set stream non-blocking");
    let read_deadline = Instant::now() + Duration::from_millis(500);
    let bytes_read = read_request(&mut stream, read_deadline);
    if bytes_read > 0 {
        write_response(&mut stream, &body);
    }
}

fn accept_connection(listener: &TcpListener, deadline: Instant) -> TcpStream {
    loop {
        match listener.accept() {
            Ok((stream, _)) => return stream,
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                assert!(
                    Instant::now() < deadline,
                    "timed out waiting for fetch test connection"
                );
                thread::sleep(Duration::from_millis(10));
            }
            Err(err) => panic!("failed to accept connection: {err}"),
        }
    }
}

fn read_request(stream: &mut TcpStream, deadline: Instant) -> usize {
    let mut buf = [0u8; 512];
    loop {
        if Instant::now() >= deadline {
            return 0;
        }

        match stream.read(&mut buf) {
            Ok(0) => return 0,
            Ok(n) => return n,
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                if !handle_would_block(deadline) {
                    return 0;
                }
            }
            Err(err) => panic!("failed to read request: {err}"),
        }
    }
}

fn handle_would_block(deadline: Instant) -> bool {
    if Instant::now() >= deadline {
        false
    } else {
        thread::sleep(Duration::from_millis(5));
        true
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
