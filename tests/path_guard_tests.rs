//! Tests for PATH restoration behaviour using mock and real environments.
//!
//! Verifies that `PathGuard` restores `PATH` without mutating the real
//! process environment, and that `PathGuard::capture` snapshots and
//! restores the real `PATH`.

use anyhow::{Context, Result, ensure};
use mockall::{Sequence, mock};
use serial_test::serial;
use std::ffi::OsStr;
use test_support::{Environment, PathGuard};

mock! {
    pub Env {}
    impl Environment for Env {
        unsafe fn set_var(&mut self, key: &str, val: &OsStr);
        unsafe fn remove_var(&mut self, key: &str);
    }
}

#[test]
fn restores_path_without_touching_real_env() {
    let mut env = MockEnv::new();
    let mut seq = Sequence::new();
    env.expect_set_var()
        .withf(|k, v| k == "PATH" && v == OsStr::new("/tmp"))
        .times(1)
        .in_sequence(&mut seq)
        .return_const(());
    env.expect_set_var()
        .withf(|k, v| k == "PATH" && v == OsStr::new("/orig"))
        .times(1)
        .in_sequence(&mut seq)
        .return_const(());
    {
        let mut guard = PathGuard::with_env(Some("/orig".into()), env);
        unsafe {
            guard.env_mut().set_var("PATH", OsStr::new("/tmp"));
        }
    }
}

#[test]
#[serial]
fn capture_snapshots_and_restores_real_path() -> Result<()> {
    let original = std::env::var_os("PATH");
    {
        let _guard = PathGuard::capture();
        test_support::env::set_var("PATH", OsStr::new("/netsuke-capture-test"));
        let mutated = std::env::var_os("PATH").context("PATH should be set after mutation")?;
        ensure!(
            mutated == OsStr::new("/netsuke-capture-test"),
            "expected mutated PATH, got {mutated:?}"
        );
    }
    ensure!(
        std::env::var_os("PATH") == original,
        "expected PATH to be restored to its captured value after guard drop"
    );
    Ok(())
}
