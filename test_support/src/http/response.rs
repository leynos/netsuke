//! HTTP response shapes emitted by the local test fixture.
//!
//! Two shapes are emitted. [`HttpResponse`] describes a valid structured
//! response, which is what most fixtures want; [`RawHttpResponse`] carries the
//! bytes themselves, for tests about what a client does when the bytes on the
//! wire are not a response any server should send.

use std::{io, io::Write, net::TcpStream};

/// Describe one response emitted by the local HTTP fixture.
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

/// Describe one response emitted by the local HTTP fixture as raw bytes.
///
/// [`HttpResponse`] is the fixture's normal shape: it renders a valid
/// response, so every field it accepts is one a real server could send. A test
/// about what a client does when the bytes on the wire are *not* valid — a
/// status line no client can parse, a truncated header block — cannot be
/// expressed that way, because the constructor refuses the very input the test
/// needs. This type carries the bytes instead, and imposes nothing on them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawHttpResponse {
    /// Bytes written to the client verbatim, in this order.
    bytes: Vec<u8>,
}

impl RawHttpResponse {
    /// Create a response that writes `bytes` to the client verbatim.
    #[must_use]
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
        }
    }

    /// Return the bytes this response writes to the client.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Write `response` to `stream` as a complete HTTP/1.1 response.
///
/// # Errors
///
/// Returns an error when the response cannot be written to the stream.
pub(super) fn write_response(stream: &mut TcpStream, response: &HttpResponse) -> io::Result<()> {
    stream.write_all(render_response(response).as_bytes())
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

/// Write `response`'s bytes to `stream` verbatim.
///
/// # Errors
///
/// Returns an error when the bytes cannot be written to the stream.
pub(super) fn write_raw_response(
    stream: &mut TcpStream,
    response: &RawHttpResponse,
) -> io::Result<()> {
    stream.write_all(&response.bytes)
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
