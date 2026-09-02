//! Integration tests for policy enforcement across fetch redirects.

use std::io;

use anyhow::{Context, Result, anyhow, bail, ensure};
use minijinja::context;
use netsuke::stdlib::{NetworkPolicy, StdlibConfig};
use rstest::rstest;
use test_support::http::{self, HttpResponse};
use url::Url;

use super::support::fallible;

/// Render a fetch template with `policy` and return its output and impurity state.
fn render_fetch(policy: NetworkPolicy, url: &str) -> Result<(String, bool)> {
    render_fetch_with_cache(policy, url, false)
}

/// Render a fetch template with a chosen cache setting.
fn render_fetch_with_cache(
    policy: NetworkPolicy,
    url: &str,
    use_cache: bool,
) -> Result<(String, bool)> {
    let (mut env, state) = fallible::stdlib_env_with_config(
        StdlibConfig::from_current_dir()?.with_network_policy(policy),
    )?;
    state.reset_impure();
    let source = if use_cache {
        "{{ fetch(url, cache=true) }}"
    } else {
        "{{ fetch(url) }}"
    };
    fallible::register_template(&mut env, "redirect_fetch", source)?;
    let template = env
        .get_template("redirect_fetch")
        .context("fetch redirect template")?;
    let rendered = template.render(context!(url => url))?;
    Ok((rendered, state.is_impure()))
}

/// Convert a loopback fixture URL into an equivalent `localhost` URL.
fn localhost_url(url: &str) -> Result<String> {
    let mut parsed = Url::parse(url).context("parse redirector URL")?;
    parsed
        .set_host(Some("localhost"))
        .map_err(|_| anyhow!("redirector URL must carry a host"))?;
    Ok(parsed.to_string())
}

/// Join a fixture server and report any thread panic.
fn join_server(server: http::HttpServer, name: &str) -> Result<()> {
    server
        .join()
        .map_err(|err| anyhow!("{name} server thread panicked: {err:?}"))
}

/// Verify a blocked redirect target receives no request.
#[rstest]
fn fetch_rejects_redirect_to_blocked_host_before_connecting() -> Result<()> {
    let (target_url, target_requests, target_server) =
        match http::spawn_http_server_responses([HttpResponse::new(200, "blocked target")]) {
            Ok(server) => server,
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                tracing::warn!("Skipping redirect-block test: cannot bind HTTP listener ({err})");
                return Ok(());
            }
            Err(err) => return Err(err).context("spawn blocked target server"),
        };
    let (redirector_loopback_url, _redirector_requests, redirector_server) =
        http::spawn_http_server_responses([
            HttpResponse::new(302, "").with_header("Location", target_url)
        ])
        .context("spawn redirector server")?;
    let policy = NetworkPolicy::default()
        .allow_scheme("http")?
        .deny_all_hosts()
        .allow_hosts(["localhost"])?
        .block_host("127.0.0.1")?;
    let redirector_url = localhost_url(&redirector_loopback_url)?;

    let err = render_fetch(policy, &redirector_url).expect_err("blocked redirect should fail");
    ensure!(
        err.to_string().contains("Redirect URL"),
        "blocked redirect should report redirect policy failure: {err}"
    );
    join_server(redirector_server, "redirector")?;
    join_server(target_server, "blocked target")?;
    ensure!(
        target_requests.load(std::sync::atomic::Ordering::Relaxed) == 0,
        "denied target must receive no request",
    );
    Ok(())
}

/// Verify default-deny rejects a redirect target outside the allowlist.
#[rstest]
fn fetch_rejects_non_allowlisted_redirect_before_connecting() -> Result<()> {
    let (target_url, target_requests, target_server) =
        match http::spawn_http_server_responses([HttpResponse::new(200, "not allowlisted")]) {
            Ok(server) => server,
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                tracing::warn!(
                    "Skipping redirect-allowlist test: cannot bind HTTP listener ({err})"
                );
                return Ok(());
            }
            Err(err) => return Err(err).context("spawn non-allowlisted target server"),
        };
    let (redirector_loopback_url, _redirector_requests, redirector_server) =
        http::spawn_http_server_responses([
            HttpResponse::new(302, "").with_header("Location", target_url)
        ])
        .context("spawn default-deny redirector server")?;
    let policy = NetworkPolicy::default()
        .allow_scheme("http")?
        .deny_all_hosts()
        .allow_hosts(["localhost"])?;
    let redirector_url = localhost_url(&redirector_loopback_url)?;

    let err =
        render_fetch(policy, &redirector_url).expect_err("non-allowlisted redirect should fail");
    ensure!(
        err.to_string().contains("not on the allowlist"),
        "default-deny redirect should preserve its policy reason: {err}"
    );
    join_server(redirector_server, "default-deny redirector")?;
    join_server(target_server, "non-allowlisted target")?;
    ensure!(
        target_requests.load(std::sync::atomic::Ordering::Relaxed) == 0,
        "non-allowlisted target must receive no request",
    );
    Ok(())
}

/// Verify allowed same-origin relative redirects preserve GET semantics.
#[rstest]
fn fetch_follows_relative_redirect_within_allowed_origin() -> Result<()> {
    let (url, requests, server) = match http::spawn_http_server_responses([
        HttpResponse::new(302, "").with_header("Location", "/next"),
        HttpResponse::new(200, "redirected body"),
    ]) {
        Ok(server) => server,
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
            tracing::warn!("Skipping relative redirect test: cannot bind HTTP listener ({err})");
            return Ok(());
        }
        Err(err) => bail!("spawn relative redirect server: {err}"),
    };
    let policy = NetworkPolicy::default().allow_scheme("http")?;

    let (rendered, impure) = render_fetch(policy, &url)?;
    join_server(server, "relative redirect")?;
    ensure!(
        rendered == "redirected body",
        "relative redirect should return its body"
    );
    ensure!(
        impure,
        "relative redirect fetch should mark the template impure"
    );
    ensure!(
        requests.load(std::sync::atomic::Ordering::Relaxed) == 2,
        "relative redirect should issue exactly two requests",
    );
    Ok(())
}

/// Assert that one cache mode rejects a blocked redirect before connecting to it.
fn assert_cache_mode_rejects_blocked_redirect(use_cache: bool) -> Result<()> {
    let (target_url, target_requests, target_server) =
        match http::spawn_http_server_responses([HttpResponse::new(200, "blocked target")]) {
            Ok(server) => server,
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => return Ok(()),
            Err(err) => return Err(err).context("spawn cached redirect target"),
        };
    let (redirector_loopback_url, _redirector_requests, redirector_server) =
        http::spawn_http_server_responses([
            HttpResponse::new(302, "").with_header("Location", target_url)
        ])
        .context("spawn cached redirector")?;
    let policy = NetworkPolicy::default()
        .allow_scheme("http")?
        .deny_all_hosts()
        .allow_hosts(["localhost"])?;
    let redirector_url = localhost_url(&redirector_loopback_url)?;

    let result = render_fetch_with_cache(policy, &redirector_url, use_cache);
    ensure!(
        result.is_err(),
        "blocked redirect should fail in both cache modes",
    );
    join_server(redirector_server, "cached redirector")?;
    join_server(target_server, "cached redirect target")?;
    ensure!(
        target_requests.load(std::sync::atomic::Ordering::Relaxed) == 0,
        "blocked target must receive no request when cache={use_cache}",
    );
    Ok(())
}

/// Verify cached and uncached fetches share redirect-policy enforcement.
#[rstest]
fn fetch_cache_modes_reject_blocked_redirects_before_connecting() -> Result<()> {
    for use_cache in [false, true] {
        assert_cache_mode_rejects_blocked_redirect(use_cache)?;
    }
    Ok(())
}

/// Verify redirects to a previously visited URL fail deterministically.
#[rstest]
fn fetch_rejects_redirect_loops() -> Result<()> {
    let (url, requests, server) = match http::spawn_http_server_responses([
        HttpResponse::new(302, "").with_header("Location", "/loop"),
        HttpResponse::new(302, "").with_header("Location", "/loop"),
    ]) {
        Ok(server) => server,
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => return Ok(()),
        Err(err) => bail!("spawn loop redirector: {err}"),
    };

    let err = render_fetch(NetworkPolicy::default().allow_scheme("http")?, &url)
        .expect_err("redirect loop should fail");
    join_server(server, "loop redirector")?;
    ensure!(
        err.to_string().contains("Redirect loop"),
        "expected loop error: {err}"
    );
    ensure!(
        requests.load(std::sync::atomic::Ordering::Relaxed) == 2,
        "redirect loop should stop before a third request",
    );
    Ok(())
}

/// Verify a redirect chain exceeding the limit fails before opening another hop.
#[rstest]
fn fetch_rejects_redirect_chains_beyond_the_limit() -> Result<()> {
    let responses = (1..=6)
        .map(|hop| HttpResponse::new(302, "").with_header("Location", format!("/hop/{hop}")));
    let (url, requests, server) = match http::spawn_http_server_responses(responses) {
        Ok(server) => server,
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => return Ok(()),
        Err(err) => bail!("spawn redirect chain server: {err}"),
    };

    let err = render_fetch(NetworkPolicy::default().allow_scheme("http")?, &url)
        .expect_err("over-limit redirect chain should fail");
    join_server(server, "redirect chain")?;
    ensure!(
        err.to_string().contains("Redirect limit"),
        "expected limit error: {err}"
    );
    ensure!(
        requests.load(std::sync::atomic::Ordering::Relaxed) == 6,
        "redirect limit should stop before a seventh request",
    );
    Ok(())
}
