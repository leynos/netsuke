//! Deliberately raw response payloads for the local HTTP fixture.
//!
//! [`HttpResponse`](super::HttpResponse) composes a response — a status line, a
//! terminating header block, and a matching `Content-Length` — which is what
//! most fixtures want. A test of a client's *parse* failures needs the opposite:
//! bytes no client accepts, emitted verbatim. This module holds that payload and
//! the completion helper that delivers it, kept separate so a case cannot
//! express malformed bytes through the type the ordinary fixtures build.

use std::{
    io::{self, Write as _},
    net::{Shutdown, TcpStream},
};

#[cfg(test)]
use std::io::Read as _;

/// Response bytes emitted verbatim by the local HTTP fixture.
///
/// The fixture writes these bytes exactly as given, so a case can present a
/// status line, header block, or framing no HTTP client accepts. Every other
/// fixture response goes through the checked
/// [`HttpResponse`](super::HttpResponse) path instead; keep a raw payload for a
/// deliberately malformed response, not for a valid one that merely needs an
/// unusual status.
///
/// Nothing here validates the bytes. A caller that wants a parse failure is
/// responsible for writing payload the client will reject, and a caller that
/// writes a valid response this way forfeits the guarantee
/// [`HttpResponse`](super::HttpResponse) provides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawHttpResponse {
    /// Response bytes emitted verbatim.
    bytes: Vec<u8>,
}

impl RawHttpResponse {
    /// Create a raw response from `bytes`.
    #[must_use]
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
        }
    }

    /// Create a raw response from a text literal.
    ///
    /// A literal byte sequence a case wants to send verbatim, such as a status
    /// line the client will reject, reads far better as `text` than as a byte
    /// string cast, so the conversion is named here rather than repeated at
    /// every call site.
    #[must_use]
    pub fn text(bytes: &str) -> Self {
        Self::new(bytes.as_bytes())
    }

    /// Return the response bytes this fixture emits.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// A fixture payload that can be written to a client as a complete response.
///
/// The fixture needs one completion path for both the checked and the raw
/// payload, so this trait carries the bytes and [`finish_response`] performs
/// the write and the write-side shutdown for either. It is crate-private: it
/// exists so the two payload types share one completion contract, not as an
/// extension point, and [`RawHttpResponse`] is what a test configures.
pub(super) trait FinishResponse {
    /// Append this payload to `bytes`, rendering it if it is not raw.
    fn append_to(&self, bytes: &mut Vec<u8>);
}

impl FinishResponse for RawHttpResponse {
    /// Append the payload exactly as supplied.
    fn append_to(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.bytes);
    }
}

/// Write `response` to `stream` and complete the response at the TCP layer.
///
/// The write-side shutdown is not an optimization; without it a test of a
/// client's failure handling races the fixture's own teardown. Dropping the
/// stream instead closes both directions at once, and a server that closes
/// while request bytes are still unread makes the platform answer with a
/// reset, which discards the response the client had not yet consumed. The
/// client then reports a transport failure — on Windows, Winsock
/// `WSAECONNABORTED` (10053) — in place of the wire-level fault the payload was
/// written to provoke. Shutting down write alone sends the end of the response
/// as a FIN while the read side stays open to drain the request, so the client
/// sees exactly the bytes above and classifies them itself.
///
/// # Errors
///
/// Returns an error when the response cannot be written to the stream, or when
/// the write-side shutdown fails.
pub(super) fn finish_response(
    stream: &mut TcpStream,
    response: &impl FinishResponse,
) -> io::Result<()> {
    let mut bytes = Vec::new();
    response.append_to(&mut bytes);
    stream.write_all(&bytes)?;
    stream.shutdown(Shutdown::Write)
}

/// Run the fixture against a real client and report what each end observed.
///
/// A fixture test owns both ends of the connection, so it must run the fixture
/// itself to see the bytes the client side actually read. This drives one
/// request through the same completion helper the server uses and returns both
/// the client's bytes and the request bytes the fixture consumed, so a case can
/// assert the payload and that the fixture read before it answered rather than
/// leaving request bytes to force a reset. Both reads are exact rather than
/// deadline-based, so nothing here depends on how fast either end runs.
///
/// # Errors
///
/// Returns an error when the loopback listener cannot be bound or accepted,
/// when the request cannot be sent or fully consumed, when the response cannot
/// be written, or when the client cannot read it back.
#[cfg(test)]
pub(super) fn rendered_exchange(
    response: &impl FinishResponse,
    request: &[u8],
) -> io::Result<(Vec<u8>, Vec<u8>)> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
    let addr = listener.local_addr()?;
    let mut client = TcpStream::connect(addr)?;
    let (mut stream, _peer) = listener.accept()?;
    client.write_all(request)?;
    let mut consumed = vec![0_u8; request.len()];
    stream.read_exact(&mut consumed)?;
    finish_response(&mut stream, response)?;

    let mut received = Vec::new();
    client.read_to_end(&mut received)?;
    Ok((received, consumed))
}
