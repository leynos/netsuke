//! Fixture lifecycle tests for the raw and checked response paths.
//!
//! Both paths complete through one helper, so the bytes each emits, the end of
//! the connection after them, and the fixture's consumption of the request are
//! pinned here. Every case drives a real client and reads exactly, so nothing is
//! inferred from elapsed time and no case needs a sleep.

use std::io::{Read as _, Write as _};
use std::net::TcpStream;

use super::{
    HttpResponse, RawHttpResponse, raw::rendered_exchange, response, spawn_http_server,
    spawn_raw_http_server,
};

/// The malformed status line whose classification this fixture exists to pin.
const MALFORMED_STATUS_LINE: &str = "HTTP/1.1 banana OK\r\nContent-Length: 0\r\n\r\n";

/// One minimal request, matching what the other fixture tests send.
const REQUEST: &[u8] = b"GET / HTTP/1.1\r\nHost: fixture\r\nConnection: close\r\n\r\n";

/// Read the whole response from the fixture at `url`.
///
/// Returning at end of stream is what makes a case raceless: the payload and
/// the end of the connection are both observed, so neither is inferred from
/// elapsed time.
fn read_response(url: &str) -> anyhow::Result<Vec<u8>> {
    let address = url
        .strip_prefix("http://")
        .ok_or_else(|| anyhow::anyhow!("fixture URL must use HTTP: {url}"))?;
    let mut stream = TcpStream::connect(address)?;
    stream.write_all(REQUEST)?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    Ok(response)
}

/// The raw path emits exactly the bytes it was given.
///
/// A malformed status line is the payload this fixture exists for, so the case
/// asserts the client reads it unchanged and reaches end of stream after it.
#[test]
fn raw_response_server_emits_its_bytes_verbatim_and_ends_the_response() -> anyhow::Result<()> {
    let (url, requests, server) =
        spawn_raw_http_server(RawHttpResponse::text(MALFORMED_STATUS_LINE))?;

    let response = read_response(&url)?;
    server
        .join()
        .map_err(|err| anyhow::anyhow!("raw fixture server panicked: {err:?}"))?;

    anyhow::ensure!(
        response == MALFORMED_STATUS_LINE.as_bytes(),
        "the raw fixture must emit its payload verbatim, got {:?}",
        String::from_utf8_lossy(&response),
    );
    anyhow::ensure!(
        requests.load(std::sync::atomic::Ordering::Relaxed) == 1,
        "the fixture must have answered the request it read",
    );
    Ok(())
}

/// A raw payload is never normalized, so the bytes are the bytes supplied.
#[test]
fn raw_response_keeps_bytes_a_checked_response_could_not_produce() {
    let arbitrary = [0x00_u8, 0xff, b'\r', b'\n', 0x7f];
    assert_eq!(
        RawHttpResponse::new(arbitrary).as_bytes(),
        arbitrary,
        "a raw payload must round-trip unchanged",
    );
}

/// The checked path still emits a well-formed response.
///
/// The raw path shares the checked path's completion, so this case confirms the
/// shared helper did not displace the checked rendering.
#[test]
fn checked_response_server_emits_a_well_formed_response_and_ends_the_response() -> anyhow::Result<()>
{
    let (url, server) = spawn_http_server("checked body")?;

    let response = read_response(&url)?;
    server
        .join()
        .map_err(|err| anyhow::anyhow!("checked fixture server panicked: {err:?}"))?;

    let rendered = String::from_utf8(response)
        .map_err(|err| anyhow::anyhow!("a checked response must be valid UTF-8: {err}"))?;
    anyhow::ensure!(
        rendered.starts_with("HTTP/1.1 200 OK\r\n"),
        "a checked response must begin with its status line, got {rendered}",
    );
    anyhow::ensure!(
        rendered.ends_with("\r\n\r\nchecked body"),
        "a checked response must carry its body after the header block, got {rendered}",
    );
    Ok(())
}

/// A checked response reaches the client byte-for-byte as it renders.
#[test]
fn checked_response_round_trips_through_the_completion_helper() -> anyhow::Result<()> {
    let response = HttpResponse::new(302, "next").with_header("Location", "/redirected");
    let (received, consumed) = rendered_exchange(&response, REQUEST)
        .map_err(|err| anyhow::anyhow!("render a checked response: {err}"))?;

    anyhow::ensure!(
        received == response::render_response(&response).as_bytes(),
        "the completion helper must emit the rendered response unchanged, got {:?}",
        String::from_utf8_lossy(&received),
    );
    anyhow::ensure!(
        consumed == REQUEST,
        "the fixture must consume the request before it answers, got {consumed:?}",
    );
    Ok(())
}

/// The helper writes a raw payload exactly, including bytes no client parses.
#[test]
fn raw_response_round_trips_through_the_completion_helper() -> anyhow::Result<()> {
    let response = RawHttpResponse::text(MALFORMED_STATUS_LINE);
    let (received, consumed) = rendered_exchange(&response, REQUEST)
        .map_err(|err| anyhow::anyhow!("render a raw response: {err}"))?;

    anyhow::ensure!(
        received == MALFORMED_STATUS_LINE.as_bytes(),
        "the completion helper must emit a raw payload verbatim, got {:?}",
        String::from_utf8_lossy(&received),
    );
    anyhow::ensure!(
        consumed == REQUEST,
        "the fixture must consume the request before it answers, got {consumed:?}",
    );
    Ok(())
}
