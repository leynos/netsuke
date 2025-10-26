//! Tests for stdlib network helpers covering fetch caching and failure paths.

use std::{fs, io};

use anyhow::{anyhow, bail, ensure, Context, Result};
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{context, Environment, ErrorKind};
use netsuke::stdlib::{NetworkPolicy, StdlibConfig};
use rstest::{fixture, rstest};
use tempfile::tempdir;

use super::support::fallible;
use test_support::{hash, http};

#[fixture]
fn http_policy() -> Result<NetworkPolicy> {
    NetworkPolicy::default().allow_scheme("http")
}

fn env_with_policy(policy: NetworkPolicy) -> Result<(Environment<'static>, netsuke::stdlib::StdlibState)> {
    fallible::stdlib_env_with_config(StdlibConfig::default().with_network_policy(policy))
}

fn env_with_workspace_policy(
    workspace: Dir,
    policy: NetworkPolicy,
) -> Result<(Environment<'static>, netsuke::stdlib::StdlibState)> {
    fallible::stdlib_env_with_config(StdlibConfig::new(workspace).with_network_policy(policy))
}

#[rstest]
fn fetch_function_downloads_content(http_policy: Result<NetworkPolicy>) -> Result<()> {
    let (url, server) = match http::spawn_http_server("payload") {
        Ok(pair) => pair,
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
            tracing::warn!(
                "Skipping fetch_function_downloads_content: cannot bind HTTP listener ({err})"
            );
            return Ok(());
        }
        Err(err) => bail!("failed to spawn HTTP server: {err}"),
    };
    let policy = http_policy?
        .block_host("169.254.169.254")?;
    let (mut env, mut state) = env_with_policy(policy)?;
    state.reset_impure();
    fallible::register_template(&mut env, "fetch", "{{ fetch(url) }}")?;
    let tmpl = env
        .get_template("fetch")
        .context("fetch template 'fetch'")?;
    let rendered = tmpl
        .render(context!(url => url.clone()))
        .context("render fetch template")?;
    ensure!(rendered == "payload", "expected payload but rendered {rendered}");
    ensure!(
        state.is_impure(),
        "network fetch should mark template impure"
    );
    server
        .join()
        .map_err(|err| anyhow!("HTTP server thread panicked: {err:?}"))?;
    Ok(())
}

#[rstest]
fn fetch_function_allows_wildcard_hosts(http_policy: Result<NetworkPolicy>) -> Result<()> {
    let (url, server) = match http::spawn_http_server("wildcard") {
        Ok(pair) => pair,
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
            tracing::warn!(
                "Skipping fetch_function_allows_wildcard_hosts: cannot bind HTTP listener ({err})"
            );
            return Ok(());
        }
        Err(err) => bail!("failed to spawn HTTP server: {err}"),
    };
    let policy = http_policy?
        .deny_all_hosts()
        .allow_hosts(["*.0.0.1"])?;
    let (mut env, mut state) = env_with_policy(policy)?;
    state.reset_impure();
    fallible::register_template(&mut env, "fetch", "{{ fetch(url) }}")?;
    let tmpl = env
        .get_template("fetch")
        .context("fetch template 'fetch'")?;
    let rendered = tmpl
        .render(context!(url => url.clone()))
        .context("render fetch template")?;
    ensure!(rendered == "wildcard", "expected wildcard but rendered {rendered}");
    ensure!(state.is_impure(), "network fetch should mark template impure");
    server
        .join()
        .map_err(|err| anyhow!("HTTP server thread panicked: {err:?}"))?;
    Ok(())
}

#[rstest]
fn fetch_function_respects_cache(http_policy: Result<NetworkPolicy>) -> Result<()> {
    let temp_dir = tempdir().context("create fetch cache tempdir")?;
    let temp_root = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
        .map_err(|path| anyhow!("temporary root is not valid UTF-8: {path:?}"))?;
    let (url, server) = match http::spawn_http_server("cached") {
        Ok(pair) => pair,
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
            tracing::warn!(
                "Skipping fetch_function_respects_cache: cannot bind HTTP listener ({err})"
            );
            return Ok(());
        }
        Err(err) => bail!("failed to spawn HTTP server: {err}"),
    };
    let workspace = Dir::open_ambient_dir(&temp_root, ambient_authority())
        .context("open fetch cache workspace")?;
    let policy = http_policy?;
    let (mut env, mut state) = env_with_workspace_policy(workspace, policy)?;
    state.reset_impure();
    fallible::register_template(&mut env, "fetch_cache", "{{ fetch(url, cache=true) }}")?;
    let tmpl = env
        .get_template("fetch_cache")
        .context("fetch template 'fetch_cache'")?;
    let rendered = tmpl
        .render(context!(url => url.clone()))
        .context("render fetch with caching")?;
    ensure!(rendered == "cached", "expected 'cached' but rendered {rendered}");
    ensure!(
        state.is_impure(),
        "network-backed cache fill should mark template impure"
    );
    state.reset_impure();
    server
        .join()
        .map_err(|err| anyhow!("HTTP server thread panicked: {err:?}"))?;

    // Drop the listener and verify the cached response is returned.
    let rendered_again = tmpl
        .render(context!(url => url.clone()))
        .context("render cached fetch")?;
    ensure!(rendered_again == "cached", "expected cached response but rendered {rendered_again}");
    ensure!(
        state.is_impure(),
        "cached responses should mark template impure",
    );

    let cache_key = hash::sha256_hex(url.as_bytes());
    let cache_path = temp_root.join(".netsuke").join("fetch").join(cache_key);
    ensure!(
        fs::metadata(cache_path.as_std_path()).is_ok(),
        "cache file should exist inside the workspace"
    );
    Ok(())
}

#[rstest]
fn fetch_function_reports_errors(http_policy: Result<NetworkPolicy>) -> Result<()> {
    let policy = http_policy?;
    let (mut env, mut state) = env_with_policy(policy)?;
    state.reset_impure();
    fallible::register_template(&mut env, "fetch_fail", "{{ fetch(url) }}")?;
    let tmpl = env
        .get_template("fetch_fail")
        .context("fetch template 'fetch_fail'")?;
    let err = match tmpl.render(context!(url => "http://127.0.0.1:9")) {
        Ok(output) => bail!(
            "expected fetch to report connection error but rendered {output}"
        ),
        Err(err) => err,
    };
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "fetch should report InvalidOperation on failure but was {:?}",
        err.kind()
    );
    ensure!(
        err.to_string().contains("fetch failed"),
        "error should mention failure: {err}"
    );
    ensure!(
        state.is_impure(),
        "failed fetch should still mark template impure",
    );
    Ok(())
}

#[rstest]
fn fetch_function_rejects_template_cache_dir(http_policy: Result<NetworkPolicy>) -> Result<()> {
    let policy = http_policy?;
    let (mut env, mut state) = env_with_policy(policy)?;
    state.reset_impure();
    fallible::register_template(
        &mut env,
        "fetch_cache_dir",
        "{{ fetch(url, cache=true, cache_dir='.netsuke/cache') }}",
    )?;
    let tmpl = env
        .get_template("fetch_cache_dir")
        .context("fetch template 'fetch_cache_dir'")?;
    let err = match tmpl.render(context!(url => "http://127.0.0.1:9")) {
        Ok(output) => bail!(
            "expected fetch to reject cache_dir override but rendered {output}"
        ),
        Err(err) => err,
    };
    ensure!(
        err.to_string().contains("cache_dir"),
        "error should mention unexpected cache_dir argument: {err}"
    );
    ensure!(
        !state.is_impure(),
        "failing before issuing a request should not mark the template impure",
    );
    Ok(())
}
