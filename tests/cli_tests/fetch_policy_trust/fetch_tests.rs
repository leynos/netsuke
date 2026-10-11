//! Exercise production `fetch()` with policy from the child-process merge.

use super::*;
use anyhow::{Context, Result, anyhow, bail, ensure};
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{Environment, context};
use netsuke::stdlib::StdlibConfig;
use std::io;
use tempfile::tempdir;
use test_support::http::{self, HttpResponse};

/// Render `fetch()` using the network policy returned by the real CLI merge.
fn render_fetch(cli: &Cli, project: &TempDir, value: &str) -> Result<String> {
    let root = Utf8PathBuf::from_path_buf(project.path().to_path_buf())
        .map_err(|path| anyhow!("project path is not UTF-8: {}", path.display()))?;
    let directory = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open merged project workspace")?;
    let config = StdlibConfig::new(directory)?
        .with_workspace_root_path(root)?
        .with_network_policy(cli.network_policy()?);
    let mut environment = Environment::new();
    netsuke::stdlib::register_with_config(&mut environment, config)
        .context("register stdlib with merged network policy")?;
    environment
        .add_template("fetch", "{{ fetch(url) }}")
        .context("register fetch template")?;
    environment
        .get_template("fetch")
        .context("load fetch template")?
        .render(context!(url => value))
        .context("render fetch template")
}

#[rstest::rstest]
fn project_chain_grants_cannot_reach_a_loopback_server_without_operator_trust() -> Result<()> {
    let (server_url, requests, server) =
        match http::spawn_http_server_expecting_no_requests(HttpResponse::new(200, "secret")) {
            Ok(fixture) => fixture,
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => return Ok(()),
            Err(err) => bail!("spawn loopback fetch fixture: {err}"),
        };
    let project = tempdir().context("create project workspace")?;
    let merged = merge_fetch_policy_in_project(
        &project,
        &FetchPolicyFileLayers {
            user: Some("fetch_default_deny = true\n"),
            project: Some("extends = \"policy.toml\"\n"),
            project_files: Some(vec![(
                "policy.toml",
                "fetch_allow_scheme = [\"http\"]\nfetch_allow_host = [\"127.0.0.1\"]\n",
            )]),
            ..FetchPolicyFileLayers::default()
        },
        &[],
        &["netsuke"],
    )?;

    let error = render_fetch(&merged, &project, &server_url)
        .expect_err("quarantined project grants must reject the loopback fetch");
    server
        .join()
        .map_err(|panic| anyhow!("loopback server thread panicked: {panic:?}"))?;
    ensure!(
        requests.lines().is_empty(),
        "policy rejection must happen before a loopback request: {:?}",
        requests.lines()
    );
    let error_chain = format!("{error:#}");
    ensure!(
        error_chain.contains("disallowed")
            && error_chain.contains("Scheme")
            && error_chain.contains("not allowed"),
        "fetch should report the scheme policy rejection: {error_chain}"
    );
    Ok(())
}

#[rstest::rstest]
fn operator_trust_allows_project_chain_fetch_to_loopback() -> Result<()> {
    let (server_url, requests, server) =
        match http::spawn_http_server_recording([HttpResponse::new(200, "fetched body")]) {
            Ok(fixture) => fixture,
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => return Ok(()),
            Err(err) => bail!("spawn loopback fetch fixture: {err}"),
        };
    let project = tempdir().context("create project workspace")?;
    let merged = merge_fetch_policy_in_project(
        &project,
        &FetchPolicyFileLayers {
            user: Some("trust_project_fetch_policy = true\nfetch_default_deny = true\n"),
            project: Some("extends = \"policy.toml\"\n"),
            project_files: Some(vec![(
                "policy.toml",
                "fetch_allow_scheme = [\"http\"]\nfetch_allow_host = [\"127.0.0.1\"]\n",
            )]),
            ..FetchPolicyFileLayers::default()
        },
        &[],
        &["netsuke"],
    )?;

    let body = render_fetch(&merged, &project, &server_url)?;
    server
        .join()
        .map_err(|panic| anyhow!("loopback server thread panicked: {panic:?}"))?;
    ensure!(body == "fetched body", "unexpected fetch body: {body}");
    ensure!(
        requests.lines().len() == 1,
        "trusted fetch must reach the loopback server exactly once: {:?}",
        requests.lines()
    );
    Ok(())
}
