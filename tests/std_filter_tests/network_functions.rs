use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

use minijinja::{ErrorKind, context};
use rstest::rstest;
use tempfile::tempdir;

use super::support::stdlib_env_with_state;

fn start_server(body: &'static str) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    let url = format!("http://{addr}");
    let handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 512];
            let bytes_read = stream.read(&mut buf).unwrap_or(0);
            if bytes_read > 0 {
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
            }
        }
    });
    (url, handle)
}

#[rstest]
fn fetch_function_downloads_content() {
    let (url, handle) = start_server("payload");
    let (mut env, state) = stdlib_env_with_state();
    state.reset_impure();
    env.add_template("fetch", "{{ fetch(url) }}")
        .expect("template");
    let tmpl = env.get_template("fetch").expect("get template");
    let rendered = tmpl
        .render(context!(url => url.clone()))
        .expect("render fetch");
    assert_eq!(rendered, "payload");
    assert!(
        state.is_impure(),
        "network fetch should mark template impure"
    );
    handle.join().expect("join server");
}

#[rstest]
fn fetch_function_respects_cache() {
    let temp = tempdir().expect("tempdir");
    let cache_dir = temp.path().join("cache");
    let cache_str = cache_dir.to_str().expect("utf8 cache dir").to_owned();
    let (url, handle) = start_server("cached");
    let (mut env, state) = stdlib_env_with_state();
    state.reset_impure();
    env.add_template(
        "fetch_cache",
        "{{ fetch(url, cache=True, cache_dir=cache_dir) }}",
    )
    .expect("template");
    let tmpl = env.get_template("fetch_cache").expect("get template");
    let rendered = tmpl
        .render(context!(url => url.clone(), cache_dir => cache_str.clone()))
        .expect("render fetch");
    assert_eq!(rendered, "cached");
    assert!(
        state.is_impure(),
        "network-backed cache fill should mark template impure"
    );
    state.reset_impure();
    handle.join().expect("join server");

    // Drop the listener and verify the cached response is returned.
    let rendered_again = tmpl
        .render(context!(url => url, cache_dir => cache_str))
        .expect("render cached fetch");
    assert_eq!(rendered_again, "cached");
    assert!(
        state.is_impure(),
        "cached responses should mark template impure",
    );
}

#[rstest]
fn fetch_function_reports_errors() {
    let (mut env, state) = stdlib_env_with_state();
    state.reset_impure();
    env.add_template("fetch_fail", "{{ fetch(url) }}")
        .expect("template");
    let tmpl = env.get_template("fetch_fail").expect("get template");
    let result = tmpl.render(context!(url => "http://127.0.0.1:9"));
    let err = result.expect_err("fetch should report connection errors");
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    assert!(
        err.to_string().contains("fetch failed"),
        "error should mention failure: {err}",
    );
    assert!(
        state.is_impure(),
        "failed fetch should still mark template impure",
    );
}
