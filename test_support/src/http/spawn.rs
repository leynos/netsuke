//! Listener setup and thread ownership for the local HTTP fixture.
//!
//! Every fixture the module exposes is built here, and every one of them is
//! built the same way: bind a loopback port, share a request counter, a request
//! log, and a shutdown flag with the caller, then move a
//! [`FixtureResponses`] into a named thread that serves the responses. What the
//! fixture emits is the only thing a caller varies.

use std::{
    io,
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize},
    },
    thread,
};

use super::{
    HttpResponse, HttpServer, HttpServerConfig, RawHttpResponse, RequestLog,
    server::{FixtureLedger, FixtureResponses, FixtureServe},
};

/// Spawn an HTTP server using `config`, emitting responses in sequence.
///
/// Returns the bound URL, the shared request count, the request log, and the
/// server handle. The public wrappers in the parent module reshape this tuple
/// for their callers, so every fixture shares one server implementation.
pub(super) fn spawn_fixture_server(
    responses: impl IntoIterator<Item = HttpResponse>,
    config: HttpServerConfig,
) -> io::Result<(String, Arc<AtomicUsize>, RequestLog, HttpServer)> {
    let response_sequence = responses.into_iter().collect::<Vec<_>>();
    spawn_fixture_thread(
        Box::new(FixtureResponses::structured(response_sequence)),
        config,
    )
}

/// Spawn an HTTP server using `config`, emitting raw responses in sequence.
///
/// The raw counterpart of [`spawn_fixture_server`], and the only place the two
/// shapes diverge: both hand a [`FixtureResponses`] to [`spawn_fixture_thread`],
/// which owns binding, accounting, and the thread.
pub(super) fn spawn_raw_fixture_server(
    responses: impl IntoIterator<Item = RawHttpResponse>,
    config: HttpServerConfig,
) -> io::Result<(String, Arc<AtomicUsize>, RequestLog, HttpServer)> {
    let response_sequence = responses.into_iter().collect::<Vec<_>>();
    spawn_fixture_thread(Box::new(FixtureResponses::raw(response_sequence)), config)
}

/// Serve `strategy` on a fresh listener, returning its URL and shared state.
///
/// This is the one implementation behind every fixture the module exposes. A
/// [`FixtureResponses`] supplies what to emit and how to advertise it; everything
/// else — binding a loopback port, the request counter and log shared with the
/// caller, the shutdown flag, the named thread, and the handle that joins or
/// signals them — is common, so no fixture shape can drift from another's
/// acceptance, accounting, or shutdown behaviour.
fn spawn_fixture_thread(
    strategy: Box<dyn FixtureServe + Send>,
    config: HttpServerConfig,
) -> io::Result<(String, Arc<AtomicUsize>, RequestLog, HttpServer)> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    listener.set_nonblocking(true)?;
    let addr = listener.local_addr()?;
    let url = strategy.advertise(addr);
    let requests = Arc::new(AtomicUsize::new(0));
    let server_requests = Arc::clone(&requests);
    let log = RequestLog::default();
    let server_log = log.clone();
    let shutdown = Arc::new(AtomicBool::new(false));
    let server_shutdown = Arc::clone(&shutdown);
    let handle = thread::Builder::new()
        .name("netsuke-http-fixture".into())
        .spawn(move || {
            strategy.drive(
                &listener,
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
