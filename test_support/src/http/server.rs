//! Request-serving implementation for the local HTTP fixture.

use std::{
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use super::{
    HttpResponse, HttpServerConfig, RawHttpResponse, RequestLog, accept_connection,
    request::read_request_line, response,
};

/// Credentials the raw fixture advertises, and the structured one omits.
///
/// A test of a credentialed fetch needs userinfo in the URL it drives, and the
/// raw shape is the one such a test reaches for, so the fixture supplies it
/// rather than making every caller paste the same userinfo into its own request.
const RAW_FIXTURE_USERINFO: &str = "redirect-user:redirect-secret";

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
pub(super) enum FixtureProgress {
    /// The client sent a request and the next response may be served.
    Continue,
    /// No further connection is accepted, because either the client
    /// disconnected or the test asked the fixture to shut down.
    Shutdown,
}

/// One accepted client connection, and the response it is owed.
///
/// The stream and the index travel together because the index is only known
/// once the request has been read: a client that connects and sends nothing
/// must not advance the sequence, so the two cannot be carried separately
/// without inviting a response for a request that never arrived.
#[derive(Debug)]
struct FixtureRequest {
    /// The accepted client connection.
    stream: TcpStream,
    /// Zero-based index, within the configured sequence, of the response owed.
    request: usize,
}

/// What a fixture emits, and how it is driven.
///
/// This is the whole of the difference between the two shapes the fixture
/// serves: a valid structured response, and bytes the caller supplies. Both are
/// driven by the one loop in [`FixtureServe::drive`], so each shape inherits
/// the same bounded accept, the same bounded request read, the same accounting,
/// and the same shutdown behaviour, rather than reimplementing them.
///
/// Implementors belong only to the local HTTP fixture, and are moved into the
/// thread that drives them; no production call site may depend on the
/// panic-oriented test failure contract.
pub(super) trait FixtureServe {
    /// Serve one response after a client sends a request.
    #[must_use]
    fn serve_one(
        &self,
        listener: &TcpListener,
        config: &HttpServerConfig,
        ledger: &FixtureLedger<'_>,
    ) -> FixtureProgress;

    /// Form the fixture URL for a bound loopback address.
    #[must_use]
    fn advertise(&self, addr: SocketAddr) -> String;

    /// Serve responses in request order until one response is not requested.
    ///
    /// The single drive loop behind every fixture shape the module exposes.
    /// It is defined here, once, rather than in each implementor, so a new
    /// shape cannot arrive with its own copy of the run's control flow.
    fn drive(&self, listener: &TcpListener, config: &HttpServerConfig, ledger: &FixtureLedger<'_>) {
        loop {
            if self.serve_one(listener, config, ledger) == FixtureProgress::Shutdown {
                return;
            }
        }
    }
}

/// The response shape a fixture emits, and the URL it advertises.
///
/// The shapes differ in exactly two ways — the bytes written, and whether the
/// advertised URL carries credentials — so both are one enum, and the one
/// [`FixtureServe`] implementation below dispatches between them.
#[derive(Debug)]
pub(super) enum FixtureResponses {
    /// Valid structured responses, advertised without credentials.
    Structured(Vec<HttpResponse>),
    /// Bytes written to the client verbatim, advertised with credentials.
    Raw(Vec<RawHttpResponse>),
}

impl FixtureResponses {
    /// Emit `responses` in request order as valid structured HTTP responses.
    #[must_use]
    pub(super) const fn structured(responses: Vec<HttpResponse>) -> Self {
        Self::Structured(responses)
    }

    /// Emit `responses` in request order as raw bytes.
    #[must_use]
    pub(super) const fn raw(responses: Vec<RawHttpResponse>) -> Self {
        Self::Raw(responses)
    }
}

impl FixtureServe for FixtureResponses {
    fn serve_one(
        &self,
        listener: &TcpListener,
        config: &HttpServerConfig,
        ledger: &FixtureLedger<'_>,
    ) -> FixtureProgress {
        let Some(mut request) = accept_request(listener, config, ledger) else {
            return FixtureProgress::Shutdown;
        };
        match self {
            Self::Structured(responses) => match responses.get(request.request) {
                Some(response) => write_fixture_response(&mut request.stream, response),
                // The sequence holds fewer responses than the requests made
                // against it, so there is nothing left to send. Ending the run
                // keeps the caller's log honest about how far the exchange got.
                None => return FixtureProgress::Shutdown,
            },
            Self::Raw(responses) => match responses.get(request.request) {
                Some(response) => {
                    write_raw_response(&mut request.stream, response);
                    finish_raw_response(&request.stream);
                }
                None => return FixtureProgress::Shutdown,
            },
        }
        FixtureProgress::Continue
    }

    fn advertise(&self, addr: SocketAddr) -> String {
        match self {
            Self::Structured(_) => format!("http://{addr}"),
            Self::Raw(_) => format!("http://{RAW_FIXTURE_USERINFO}@{addr}"),
        }
    }
}

/// Accept one client connection and read the request it sent.
///
/// Returns `None` on the same two conditions as the helpers it composes: no
/// client connected before the test shut the fixture down, or the client
/// disconnected before sending a request. The latter is how an abandoned chain
/// is detected, so a later response cannot wait on a connection that will never
/// arrive.
///
/// The request is read to a non-empty line *before* any response goes out. That
/// ordering matters, and exists to keep a transport abort from being mistaken
/// for a protocol verdict: a fixture that closes a socket without draining the
/// request can have its close turned into a connection abort by the peer's
/// stack — Windows does exactly this — which the client reports as an I/O
/// failure before it has parsed a status line no parser accepts. The test
/// asserting a parse failure would then be asserting whatever the platform's
/// close semantics happened to do. Draining first removes the unread data, so
/// the bytes are received whole and parsed. A well-formed status line sent this
/// way still arrives; only a malformed one fails, which is the failure under
/// test.
fn accept_request(
    listener: &TcpListener,
    config: &HttpServerConfig,
    ledger: &FixtureLedger<'_>,
) -> Option<FixtureRequest> {
    let mut stream = accept_fixture_connection(listener, config, ledger.shutdown)?;
    configure_fixture_stream(&stream);
    let line = read_request_line(&mut stream, config.read_deadline(), config.poll_interval)?;
    // Exactly one request line is recorded per accepted connection, and the
    // fixture serves one connection at a time, so the log's length before this
    // line is recorded is the index of the response this request is owed.
    let request = ledger.log.len();
    ledger.log.record(line);
    ledger.requests.fetch_add(1, Ordering::Relaxed);
    Some(FixtureRequest { stream, request })
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
/// A shutdown, not a close: the read half stays open, so the client still has a
/// usable socket to parse from, and the bytes already written are delivered
/// rather than abandoned to whatever a close does to a socket still holding
/// unread peer data.
///
/// A failure is fatal for every cause except the peer having gone, which is not
/// the fixture's doing and not a verdict on the bytes it sent. A client that
/// has already reset the connection — an abandoned redirect hop, a test that
/// stopped reading — leaves nothing to half-close, and the platform reports
/// that as a transport error (`WSAECONNABORTED` or `ENOTCONN` on Windows, a
/// reset or a disconnection on Unix). Panicking there would turn the client's
/// own departure into a fixture failure, which is the same conflation of
/// transport outcome with protocol verdict this fixture exists to remove. Any
/// other failure means the fixture could not frame what it wrote — the one
/// other reason a client could see a short response — so it still panics.
fn finish_raw_response(stream: &TcpStream) {
    if let Err(err) = stream.shutdown(Shutdown::Write) {
        assert!(
            peer_is_gone(&err),
            "failed to shut down the raw fixture response: {err}"
        );
    }
}

/// Report whether `err` is the peer having already left the connection.
///
/// Deliberately narrow. `BrokenPipe` is excluded: a peer that closed its read
/// half is still there to be answered, so a broken write is the fixture failing
/// to deliver, not the client departing, and must stay fatal.
pub(super) fn peer_is_gone(err: &std::io::Error) -> bool {
    use std::io::ErrorKind;

    matches!(
        err.kind(),
        ErrorKind::NotConnected | ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted
    )
}
