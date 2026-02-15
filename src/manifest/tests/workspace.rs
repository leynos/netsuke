//! Tests covering manifest workspace resolution and filesystem helpers.
use super::super::{from_path_with_policy, stdlib_config_for_manifest};
use crate::ast::Recipe;
use crate::stdlib::NetworkPolicy;
use anyhow::{Context, Result as AnyResult, anyhow, ensure};
use camino::Utf8Path;
use rstest::rstest;
use std::fs;
use std::path::Path;
use tempfile::tempdir;
use test_support::{EnvVarGuard, env_lock::EnvLock, hash, http};
use url::Url;

struct CurrentDirGuard {
    original: std::path::PathBuf,
    _lock: EnvLock,
}

impl CurrentDirGuard {
    fn change_to(path: &std::path::Path) -> AnyResult<Self> {
        let lock = EnvLock::acquire();
        let original = std::env::current_dir().context("capture current working directory")?;
        std::env::set_current_dir(path)
            .with_context(|| format!("switch to working directory {}", path.display()))?;
        Ok(Self {
            original,
            _lock: lock,
        })
    }
}

impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        if let Err(err) = std::env::set_current_dir(&self.original) {
            tracing::warn!(
                "failed to restore working directory to {}: {err}",
                self.original.display()
            );
        }
    }
}

#[rstest]
#[case(true)]
#[case(false)]
fn stdlib_config_for_manifest_resolves_workspace_root(#[case] use_relative: bool) -> AnyResult<()> {
    let temp = tempdir().context("create temp workspace")?;
    let _guard = if use_relative {
        Some(CurrentDirGuard::change_to(temp.path())?)
    } else {
        None
    };
    let manifest_path = if use_relative {
        Path::new("Netsukefile").to_path_buf()
    } else {
        temp.path().join("Netsukefile")
    };
    let config = stdlib_config_for_manifest(&manifest_path, NetworkPolicy::default())?;
    let recorded = config
        .workspace_root_path()
        .context("workspace root path should be recorded")?;
    let expected =
        Utf8Path::from_path(temp.path()).context("temp workspace path should be valid UTF-8")?;
    ensure!(
        recorded == expected,
        "expected workspace root {expected}, got {recorded}"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn stdlib_config_for_manifest_rejects_non_utf_workspace_root() -> AnyResult<()> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let temp = tempdir().context("create temp workspace")?;
    let invalid_component = OsString::from_vec(vec![0xFF]); // invalid standalone byte
    let manifest_dir = temp.path().join(&invalid_component);
    fs::create_dir_all(&manifest_dir)
        .context("create manifest directory with invalid UTF-8 component")?;
    let manifest_path = manifest_dir.join("manifest.yml");
    let err = stdlib_config_for_manifest(&manifest_path, NetworkPolicy::default())
        .expect_err("config should fail when workspace root contains non-UTF-8 components");
    ensure!(
        err.to_string().contains("path is not valid UTF-8"),
        "error should mention non-UTF-8 components but was {err}"
    );
    Ok(())
}

#[rstest]
fn from_path_uses_manifest_directory_for_caches() -> AnyResult<()> {
    let temp = tempdir()?;
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(&workspace)?;
    let outside = temp.path().join("outside");
    fs::create_dir_all(&outside)?;
    let manifest_path = workspace.join("Netsukefile");

    let (url, server) = match http::spawn_http_server("workspace-body") {
        Ok(pair) => pair,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            tracing::warn!(
                "Skipping from_path_uses_manifest_directory_for_caches: cannot bind HTTP listener ({err})"
            );
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };
    let manifest_yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: fetch\n",
        "    vars:\n",
        "      url: \"{{ env('NETSUKE_MANIFEST_URL') }}\"\n",
        "    command: \"{{ fetch(url, cache=true) }}\"\n",
    );
    fs::write(&manifest_path, manifest_yaml)?;

    let _cwd_guard = CurrentDirGuard::change_to(&outside)?;
    let _url_guard = EnvVarGuard::set("NETSUKE_MANIFEST_URL", &url);

    let manifest = from_path_with_policy(
        &manifest_path,
        NetworkPolicy::default()
            .deny_all_hosts()
            .allow_hosts(["127.0.0.1", "localhost"])?
            .allow_scheme("http")?,
        None,
    )?;
    if let Err(err) = server.join() {
        return Err(anyhow!("join server thread panicked: {err:?}"));
    }

    let first_target = manifest.targets.first().context("target missing")?;
    match &first_target.recipe {
        Recipe::Command { command } => anyhow::ensure!(
            command == "workspace-body",
            "unexpected recipe output: {command}"
        ),
        other => anyhow::bail!("expected command recipe, got {other:?}"),
    }

    let parsed_url = Url::parse(&url).context("parse manifest URL")?;
    let cache_key = hash::sha256_hex(parsed_url.as_str().as_bytes());
    let cache_path = workspace.join(".netsuke").join("fetch").join(cache_key);
    anyhow::ensure!(
        cache_path.exists(),
        "cache file should be created inside the manifest workspace"
    );
    anyhow::ensure!(
        !outside.join(".netsuke").exists(),
        "outside working directory must not receive cache data"
    );

    Ok(())
}
