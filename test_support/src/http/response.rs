//! HTTP response shapes emitted by the local test fixture.
//!
//! [`HttpResponse`] is the composed payload: it renders a status line, a
//! terminating header block, and a `Content-Length` that always matches the body
//! it carries, which is what most fixtures need. It does not validate its
//! inputs, so it is not a guarantee against a caller passing a status that is
//! not three digits or a header value containing a line break. A test of a
//! client's parse failures wants bytes no client accepts and should use
//! [`raw`](super::raw) instead, where the bytes are the caller's own.

use std::{io, net::TcpStream};

use super::raw::{FinishResponse, RawHttpResponse, finish_response};

/// One payload the fixture can serve, checked or raw.
///
/// The server stores its configured responses behind this trait so that a
/// sequence mixing a checked rendering with deliberately malformed bytes needs
/// no second server implementation. The sequence moves to the fixture thread, so
/// the trait is `Send`; it is crate-private and sealed in practice, since only
/// [`HttpResponse`] and [`RawHttpResponse`] implement it.
pub(super) trait FixtureResponse: Send {
    /// Write this payload to `stream` and complete the response.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload cannot be written or its write-side
    /// shutdown fails.
    fn write_to(&self, stream: &mut TcpStream) -> io::Result<()>;
}

impl FixtureResponse for HttpResponse {
    fn write_to(&self, stream: &mut TcpStream) -> io::Result<()> {
        finish_response(stream, self)
    }
}

impl FixtureResponse for RawHttpResponse {
    fn write_to(&self, stream: &mut TcpStream) -> io::Result<()> {
        finish_response(stream, self)
    }
}

/// Describe one response emitted by the local HTTP fixture.
///
/// Every instance renders a status line, a header block ending in a blank line,
/// and a `Content-Length` matching its body, so the framing is always complete.
/// Status and header values are taken as given and are not validated: a case
/// that needs bytes the client is meant to reject uses
/// [`RawHttpResponse`](super::RawHttpResponse) instead of trying to express them
/// here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// HTTP status code returned to the client.
    status: u16,
    /// Headers returned to the client in insertion order.
    headers: Vec<(String, String)>,
    /// Response body returned after the headers.
    body: String,
}

impl HttpResponse {
    /// Create a response with `status` and `body`.
    #[must_use]
    pub fn new(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.into(),
        }
    }

    /// Add a response header.
    #[must_use]
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

impl FinishResponse for HttpResponse {
    /// Append the checked response rendered as a complete HTTP/1.1 response.
    fn append_to(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(render_response(self).as_bytes());
    }
}

/// Render `response` as a complete HTTP/1.1 response.
pub(super) fn render_response(response: &HttpResponse) -> String {
    let mut headers = String::new();
    for (name, value) in &response.headers {
        headers.push_str(name);
        headers.push_str(": ");
        headers.push_str(value);
        headers.push_str("\r\n");
    }
    format!(
        "HTTP/1.1 {} {}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{}",
        response.status,
        reason_phrase(response.status),
        response.body.len(),
        response.body
    )
}

/// Return the standard reason phrase for fixture status codes.
const fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        _ => "Test Response",
    }
}
