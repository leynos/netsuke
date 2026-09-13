//! Request-serving implementation for the local HTTP fixture.

use std::{
    net::{TcpListener, TcpStream},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use super::{
    HttpResponse, HttpServerConfig, RequestLog, accept_connection, request::read_request_line,
    response,
};

/// What one fixture run records about the requests it answers, and the state it
/// shares with the test that owns it.
///
/// The counter, the log, and the shutdown flag are grouped so the server thread
/// takes one argument for all three, keeping every fixture helper within the
/// argument-count limit.
#[derive(Debug, Clone, Copy)]
pub(super) struct FixtureLedger<'run> {
    /// Number of requests the fixture has answered.
    requests: &'run AtomicUsize,
    /// Request lines the fixture has answered, in arrival order.
    log: &'run RequestLog,
    /// Set by the owning handle to stop the fixture accepting connections.
    shutdown: &'run AtomicBool,
}

impl<'run> FixtureLedger<'run> {
    /// Group the request counter, request log, and shutdown flag for one run.
    #[must_use]
    pub(super) const fn new(
        requests: &'run AtomicUsize,
        log: &'run RequestLog,
        shutdown: &'run AtomicBool,
    ) -> Self {
        Self {
            requests,
            log,
            shutdown,
        }
    }
}

/// Report whether the fixture should keep serving configured responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FixtureProgress {
    /// The client sent a request and the next response may be served.
    Continue,
    /// No further connection is accepted, because either the client
    /// disconnected or the test asked the fixture to shut down.
    Shutdown,
}

/// Serve the configured responses in request order until one is not requested.
pub(super) fn run_http_server(
    listener: &TcpListener,
    responses: &[HttpResponse],
    config: &HttpServerConfig,
    ledger: &FixtureLedger<'_>,
) {
    for response in responses {
        if serve_fixture_response(listener, response, config, ledger) == FixtureProgress::Shutdown {
            return;
        }
    }
}

/// Serve one fixture response after a client sends a non-empty request.
///
/// Returns [`FixtureProgress::Shutdown`] when the client disconnects before
/// sending a request, so an abandoned chain cannot leave later responses
/// waiting on a connection that will never arrive, and likewise when the test
/// shuts the fixture down before any client connects.
///
/// This helper belongs only to the local HTTP fixture: `run_http_server`
/// composes it once for every configured response, and no production call site
/// may depend on its panic-oriented test failure contract.
#[must_use]
fn serve_fixture_response(
    listener: &TcpListener,
    response: &HttpResponse,
    config: &HttpServerConfig,
    ledger: &FixtureLedger<'_>,
) -> FixtureProgress {
    let Some(mut stream) = accept_fixture_connection(listener, config, ledger.shutdown) else {
        return FixtureProgress::Shutdown;
    };
    configure_fixture_stream(&stream);
    let Some(line) = read_request_line(&mut stream, config.read_deadline(), config.poll_interval)
    else {
        return FixtureProgress::Shutdown;
    };
    ledger.log.record(line);
    ledger.requests.fetch_add(1, Ordering::Relaxed);
    write_fixture_response(&mut stream, response);
    FixtureProgress::Continue
}

/// Accept one client connection using the fixture configuration.
///
/// Returns `None` once `shutdown` is set, which ends the run on the signal
/// alone rather than on the wake-up connection a join sends.
fn accept_fixture_connection(
    listener: &TcpListener,
    config: &HttpServerConfig,
    shutdown: &AtomicBool,
) -> Option<TcpStream> {
    accept_connection(
        listener,
        config.accept_wait(shutdown),
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
