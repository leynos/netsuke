//! Tests for scoped manipulation of `PATH` via `prepend_dir_to_path` and
//! `PathGuard`.

use anyhow::{Context, Result, ensure};
use mockable::Env;
use rstest::rstest;
use serial_test::serial;
use std::ffi::OsStr;
use test_support::env::{SystemEnv, VarGuard, mocked_path_env, prepend_dir_to_path};

#[rstest]
#[serial]
fn prepend_dir_to_path_sets_and_restores() -> Result<()> {
    let env = mocked_path_env();
    let original = env.raw("PATH").context("mock PATH should be set")?;
    let dir = tempfile::tempdir().context("create temp dir")?;
    let guard = prepend_dir_to_path(&env, dir.path())?;
    let after = std::env::var("PATH").context("read PATH after prepend")?;
    let mut split_paths = std::env::split_paths(&after);
    let first = split_paths
        .next()
        .context("PATH should contain at least one entry after prepend")?;
    ensure!(
        first == dir.path(),
        "expected {} to be first PATH entry, got {}",
        dir.path().display(),
        first.display()
    );
    drop(guard);
    let restored = std::env::var("PATH").context("read restored PATH")?;
    ensure!(
        restored == original,
        "expected restored PATH to equal original value"
    );
    Ok(())
}

#[rstest]
#[serial]
fn prepend_dir_to_path_handles_empty_path() -> Result<()> {
    let _path_guard = VarGuard::set("PATH", OsStr::new(""));
    let env = SystemEnv::new();
    let dir = tempfile::tempdir().context("create temp dir")?;
    let guard = prepend_dir_to_path(&env, dir.path())?;
    let after = std::env::var_os("PATH").context("read PATH after prepend")?;
    let paths = std::env::split_paths(&after)
        .filter(|p| !p.as_os_str().is_empty())
        .collect::<Vec<_>>();
    ensure!(
        paths == vec![dir.path().to_path_buf()],
        "expected PATH to contain only {}; got {paths:?}",
        dir.path().display()
    );
    drop(guard);
    ensure!(
        std::env::var_os("PATH") == Some(std::ffi::OsString::new()),
        "expected PATH to reset to empty after guard drop"
    );
    Ok(())
}

#[rstest]
#[serial]
fn prepend_dir_to_path_handles_missing_path() -> Result<()> {
    let _path_guard = VarGuard::unset("PATH");
    let env = SystemEnv::new();
    let dir = tempfile::tempdir().context("create temp dir")?;
    let guard = prepend_dir_to_path(&env, dir.path())?;
    let after = std::env::var_os("PATH")
        .context("PATH should exist after prepend when original variable absent")?;
    let paths: Vec<_> = std::env::split_paths(&after).collect();
    ensure!(
        paths == vec![dir.path().to_path_buf()],
        "expected PATH to contain only {}; got {paths:?}",
        dir.path().display()
    );
    drop(guard);
    ensure!(
        std::env::var_os("PATH").is_none(),
        "expected PATH to be removed after guard drop"
    );
    Ok(())
}
