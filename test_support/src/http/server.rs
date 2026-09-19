//! Request-serving implementation for the local HTTP fixture.

use std::{
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use super::{
    HttpResponse, HttpServerConfig, RawHttpResponse, RequestLog, accept_connection,
    request::read_request_line, response,
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

/// What a fixture emits, and how it marks the end of what it emits.
///
/// This is the whole of the difference between the two public shapes the
/// fixture serves: a valid structured response, and bytes the caller supplies.
/// Both are driven by the one loop below, so each shape inherits the same
/// bounded accept, the same bounded request read, the same accounting, and the
/// same shutdown behaviour, rather than reimplementing them.
///
/// Implementors belong only to the local HTTP fixture, and are moved into the
/// thread that drives them; no production call site may depend on the
/// panic-oriented test failure contract.
pub(super) trait DriveStrategy {
    /// Serve the sequence in request order until one response is not requested.
    fn drive(&self, listener: &TcpListener, config: &HttpServerConfig, ledger: &FixtureLedger<'_>);

    /// Form the fixture URL for a bound loopback address.
    fn advertise(&self, addr: SocketAddr) -> String;
}

/// Serve `responses` in request order, as valid structured HTTP responses.
#[derive(Debug)]
pub(super) struct StructuredResponses {
    /// The responses to emit, in arrival order.
    responses: Vec<HttpResponse>,
}

impl StructuredResponses {
    /// Serve `responses` in request order.
    #[must_use]
    pub(super) const fn new(responses: Vec<HttpResponse>) -> Self {
        Self { responses }
    }
}

impl DriveStrategy for StructuredResponses {
    fn drive(&self, listener: &TcpListener, config: &HttpServerConfig, ledger: &FixtureLedger<'_>) {
        for response in &self.responses {
            if serve_fixture_response(listener, response, config, ledger)
                == FixtureProgress::Shutdown
            {
                return;
            }
        }
    }

    fn advertise(&self, addr: SocketAddr) -> String {
        format!("http://{addr}")
    }
}

/// Serve `responses` in request order, as raw bytes.
///
/// The advertised URL carries the credentials the structured shape omits. A
/// test of a credentialed fetch needs userinfo in the URL it drives, and this
/// is the shape such a test reaches for, so the fixture supplies it rather
/// than making every caller paste the same userinfo into its own request.
#[derive(Debug)]
pub(super) struct RawResponses {
    /// The responses to emit, in arrival order.
    responses: Vec<RawHttpResponse>,
}

impl RawResponses {
    /// Serve `responses` in request order.
    #[must_use]
    pub(super) const fn new(responses: Vec<RawHttpResponse>) -> Self {
        Self { responses }
    }
}

impl DriveStrategy for RawResponses {
    fn drive(&self, listener: &TcpListener, config: &HttpServerConfig, ledger: &FixtureLedger<'_>) {
        for response in &self.responses {
            if serve_raw_response(listener, response, config, ledger) == FixtureProgress::Shutdown {
                return;
            }
        }
    }

    fn advertise(&self, addr: SocketAddr) -> String {
        format!("http://redirect-user:redirect-secret@{addr}")
    }
}

/// Serve one structured fixture response after a client sends a request.
///
/// Returns [`FixtureProgress::Shutdown`] when the client disconnects before
/// sending a request, so an abandoned chain cannot leave later responses
/// waiting on a connection that will never arrive, and likewise when the test
/// shuts the fixture down before any client connects.
///
/// This helper belongs only to the local HTTP fixture: the drive loop
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

/// Serve one raw fixture response after a client sends a request.
///
/// Returns [`FixtureProgress::Shutdown`] on the same two conditions as
/// [`serve_fixture_response`], reached by the same two helpers, so the two
/// shapes differ only in what they write.
///
/// The request is read to a non-empty line *before* the bytes go out, and the
/// write half is then shut down. Both matter, and both exist to keep a
/// transport abort from being mistaken for a protocol verdict: a fixture that
/// closes a socket without draining the request can have its close turned into
/// a connection abort by the peer's stack — Windows does exactly this — which
/// the client reports as an I/O failure before it has parsed a status line no
/// parser accepts. The test asserting a parse failure would then be asserting
/// whatever the platform's close semantics happened to do. Draining first
/// removes the unread data, and the half-close tells the client the response
/// ended while leaving the read path intact, so the bytes are received whole
/// and parsed. A well-formed status line sent this way still arrives; only a
/// malformed one fails, which is the failure under test.
#[must_use]
fn serve_raw_response(
    listener: &TcpListener,
    response: &RawHttpResponse,
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
    write_raw_response(&mut stream, response);
    finish_raw_response(&stream);
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

/// Write one raw response to a fixture client stream.
#[expect(
    clippy::panic,
    reason = "test HTTP helper should fail fast when response writing fails"
)]
fn write_raw_response(stream: &mut TcpStream, response: &RawHttpResponse) {
    if let Err(err) = response::write_raw_response(stream, response) {
        panic!("failed to write raw fixture response: {err}");
    }
}

/// Mark the end of a raw response by shutting down the stream's write half.
///
/// A shutdown, not a close: the read half stays open, so the client still has
/// a usable socket to parse from. See [`serve_raw_response`] for why the
/// fixture needs this at all, and why a failure here is fatal rather than
/// ignored — a fixture that did not half-close is the only other reason the
/// client could see a short response, and it should fail loudly.
#[expect(
    clippy::panic,
    reason = "test HTTP helper should fail fast when the response cannot be framed"
)]
fn finish_raw_response(stream: &TcpStream) {
    if let Err(err) = stream.shutdown(Shutdown::Write) {
        panic!("failed to shut down the raw fixture response: {err}");
    }
}
