//! Tests for stdlib network helpers covering fetch caching and failure paths.

use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

use std::fs;

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{ErrorKind, context};
use netsuke::stdlib::StdlibConfig;
use rstest::rstest;
use sha2::{Digest, Sha256};
use tempfile::tempdir;

use super::support::{stdlib_env_with_config, stdlib_env_with_state};

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
    let temp_dir = tempdir().expect("tempdir");
    let temp_root =
        Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).expect("utf8 temp path");
    let (url, handle) = start_server("cached");
    let workspace = Dir::open_ambient_dir(&temp_root, ambient_authority()).expect("workspace");
    let config = StdlibConfig::new(workspace);
    let (mut env, state) = stdlib_env_with_config(config);
    state.reset_impure();
    env.add_template("fetch_cache", "{{ fetch(url, cache=true) }}")
        .expect("template");
    let tmpl = env.get_template("fetch_cache").expect("get template");
    let rendered = tmpl
        .render(context!(url => url.clone()))
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
        .render(context!(url => url.clone()))
        .expect("render cached fetch");
    assert_eq!(rendered_again, "cached");
    assert!(
        state.is_impure(),
        "cached responses should mark template impure",
    );

    let cache_key = {
        let digest = Sha256::digest(url.as_bytes());
        let mut key = String::with_capacity(digest.len() * 2);
        for byte in digest {
            use std::fmt::Write;
            let _ = write!(&mut key, "{byte:02x}");
        }
        key
    };
    let cache_path = temp_root.join(".netsuke").join("fetch").join(cache_key);
    assert!(
        fs::metadata(cache_path.as_std_path()).is_ok(),
        "cache file should exist inside the workspace",
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

#[rstest]
fn fetch_function_rejects_template_cache_dir() {
    let (mut env, state) = stdlib_env_with_state();
    state.reset_impure();
    env.add_template(
        "fetch_cache_dir",
        "{{ fetch(url, cache=true, cache_dir='.netsuke/cache') }}",
    )
    .expect("template");
    let tmpl = env.get_template("fetch_cache_dir").expect("get template");
    let err = tmpl
        .render(context!(url => "http://127.0.0.1:9"))
        .expect_err("cache_dir keyword should be rejected");
    assert!(
        err.to_string().contains("cache_dir"),
        "error should mention unexpected cache_dir argument: {err}",
    );
    assert!(
        !state.is_impure(),
        "failing before issuing a request should not mark the template impure",
    );
}
