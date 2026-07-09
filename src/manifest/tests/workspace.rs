//! Tests covering manifest workspace resolution and filesystem helpers.
use super::super::{from_path_with_policy, open_manifest_workspace};
use crate::ast::Recipe;
use crate::stdlib::NetworkPolicy;
use anyhow::{Context, Result as AnyResult, anyhow, ensure};
use camino::Utf8Path;
use rstest::rstest;
use std::path::Path;
use tempfile::tempdir;
use test_support::fs as test_fs;
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
fn open_manifest_workspace_resolves_workspace_root(#[case] use_relative: bool) -> AnyResult<()> {
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
    let workspace = open_manifest_workspace(&manifest_path)?;
    let expected =
        Utf8Path::from_path(temp.path()).context("temp workspace path should be valid UTF-8")?;
    ensure!(
        workspace.root == expected,
        "expected workspace root {expected}, got {root}",
        root = workspace.root
    );
    ensure!(
        workspace.manifest_file == "Netsukefile",
        "expected manifest file name Netsukefile, got {file}",
        file = workspace.manifest_file
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn open_manifest_workspace_rejects_non_utf_workspace_root() -> AnyResult<()> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let temp = tempdir().context("create temp workspace")?;
    let invalid_component = OsString::from_vec(vec![0xFF]); // invalid standalone byte
    let manifest_dir = temp.path().join(&invalid_component);
    test_fs::create_dir_all(&manifest_dir)
        .context("create manifest directory with invalid UTF-8 component")?;
    let manifest_path = manifest_dir.join("manifest.yml");
    let err = open_manifest_workspace(&manifest_path)
        .expect_err("workspace should fail when its root contains non-UTF-8 components");
    ensure!(
        err.to_string().contains("path is not valid UTF-8"),
        "error should mention non-UTF-8 components but was {err}"
    );
    Ok(())
}

#[rstest]
fn open_manifest_workspace_reports_missing_file_name() -> AnyResult<()> {
    // The filesystem root has no file-name component, so extraction fails with a
    // missing-name error, distinct from the non-UTF-8 case.
    let err = open_manifest_workspace(Path::new("/"))
        .expect_err("workspace should fail when the path has no file name");
    ensure!(
        err.to_string().contains("has no file name"),
        "error should report the missing file name but was {err}"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn open_manifest_workspace_rejects_non_utf_file_name() -> AnyResult<()> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let temp = tempdir().context("create temp workspace")?;
    let invalid_name = OsString::from_vec(vec![b'm', 0xFF]); // invalid trailing byte
    let manifest_path = temp.path().join(&invalid_name);
    let err = open_manifest_workspace(&manifest_path)
        .expect_err("workspace should fail when the file name is not valid UTF-8");
    ensure!(
        err.to_string().contains("path is not valid UTF-8"),
        "error should mention the non-UTF-8 file name but was {err}"
    );
    Ok(())
}

#[rstest]
fn open_manifest_workspace_reports_open_failure() -> AnyResult<()> {
    // The parent directory does not exist, so `Dir::open_ambient_dir` fails and
    // the error is wrapped as a workspace open failure.
    let temp = tempdir().context("create temp workspace")?;
    let manifest_path = temp.path().join("missing-subdir").join("Netsukefile");
    let err = open_manifest_workspace(&manifest_path)
        .expect_err("workspace open should fail when the parent directory is absent");
    ensure!(
        err.to_string().contains("Failed to open workspace"),
        "error should mention the workspace open failure but was {err}"
    );
    Ok(())
}

#[rstest]
fn from_path_uses_manifest_directory_for_caches() -> AnyResult<()> {
    let temp = tempdir()?;
    let workspace = temp.path().join("workspace");
    test_fs::create_dir_all(&workspace)?;
    let outside = temp.path().join("outside");
    test_fs::create_dir_all(&outside)?;
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
    test_fs::write(&manifest_path, manifest_yaml)?;

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
