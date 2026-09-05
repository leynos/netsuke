//! HTTP response shapes emitted by the local test fixture.

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
