//! Request-serving implementation for the local HTTP fixture.

use std::{
    net::{TcpListener, TcpStream},
    sync::atomic::{AtomicUsize, Ordering},
};

use super::{HttpResponse, HttpServerConfig, accept_connection, read_request, response};

/// Serve the configured responses in request order.
pub(super) fn run_http_server(
    listener: &TcpListener,
    responses: &[HttpResponse],
    config: &HttpServerConfig,
    requests: &AtomicUsize,
) {
    for response in responses {
        serve_fixture_response(listener, response, config, requests);
    }
}

/// Serve one fixture response after a client sends a non-empty request.
///
/// This helper belongs only to the local HTTP fixture: `run_http_server`
/// composes it once for every configured response, and no production call site
/// may depend on its panic-oriented test failure contract.
fn serve_fixture_response(
    listener: &TcpListener,
    response: &HttpResponse,
    config: &HttpServerConfig,
    requests: &AtomicUsize,
) {
    let mut stream = accept_fixture_connection(listener, config);
    configure_fixture_stream(&stream);
    if read_request(&mut stream, config.read_deadline(), config.poll_interval) == 0 {
        return;
    }
    requests.fetch_add(1, Ordering::Relaxed);
    write_fixture_response(&mut stream, response);
}

/// Accept one client connection using the fixture configuration.
fn accept_fixture_connection(listener: &TcpListener, config: &HttpServerConfig) -> TcpStream {
    accept_connection(
        listener,
        config.accept_deadline(),
        config.poll_interval,
        config.accept_timeout,
    )
}

/// Configure a fixture client stream for deadline-polled request reads.
#[expect(
    clippy::panic,
    reason = "test HTTP helper should fail fast when stream setup fails"
)]
fn configure_fixture_stream(stream: &TcpStream) {
    if let Err(err) = stream.set_nonblocking(true) {
        panic!("failed to configure stream non-blocking: {err}");
    }
}

/// Write one configured response to a fixture client stream.
#[expect(
    clippy::panic,
    reason = "test HTTP helper should fail fast when response writing fails"
)]
fn write_fixture_response(stream: &mut TcpStream, response: &HttpResponse) {
    if let Err(err) = response::write_response(stream, response) {
        panic!("failed to write fixture response: {err}");
    }
}
