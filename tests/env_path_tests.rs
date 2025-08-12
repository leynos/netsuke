use mockable::{DefaultEnv as SystemEnv, Env};
use rstest::rstest;
use serial_test::serial;

#[path = "support/env.rs"]
mod env;
mod support;
use env::{mocked_path_env, prepend_dir_to_path};
use support::env_lock::EnvLock;

#[rstest]
#[serial]
fn prepend_dir_to_path_sets_and_restores() {
    let env = mocked_path_env();
    let original = env.raw("PATH").expect("PATH should be set in mock");
    let dir = tempfile::tempdir().expect("temp dir");
    let guard = prepend_dir_to_path(&env, dir.path());
    let after = std::env::var("PATH").expect("path var");
    let first = std::env::split_paths(&std::ffi::OsString::from(&after))
        .next()
        .expect("first path");
    assert_eq!(first, dir.path());
    drop(guard);
    let restored = std::env::var("PATH").expect("path var");
    assert_eq!(restored, original);
}

#[rstest]
#[serial]
fn prepend_dir_to_path_handles_empty_path() {
    let original = std::env::var_os("PATH");
    {
        let _lock = EnvLock::acquire();
        unsafe { std::env::set_var("PATH", "") };
    }
    let env = SystemEnv::new();
    let dir = tempfile::tempdir().expect("temp dir");
    let guard = prepend_dir_to_path(&env, dir.path());
    let after = std::env::var_os("PATH").expect("path var");
    let paths = std::env::split_paths(&after)
        .filter(|p| !p.as_os_str().is_empty())
        .collect::<Vec<_>>();
    assert_eq!(paths, vec![dir.path().to_path_buf()]);
    drop(guard);
    assert_eq!(std::env::var_os("PATH"), Some(std::ffi::OsString::new()));
    {
        let _lock = EnvLock::acquire();
        if let Some(path) = original {
            unsafe { std::env::set_var("PATH", path) };
        } else {
            unsafe { std::env::remove_var("PATH") };
        }
    }
}

#[rstest]
#[serial]
fn prepend_dir_to_path_handles_missing_path() {
    let original = std::env::var_os("PATH");
    {
        let _lock = EnvLock::acquire();
        unsafe { std::env::remove_var("PATH") };
    }
    let env = SystemEnv::new();
    let dir = tempfile::tempdir().expect("temp dir");
    let guard = prepend_dir_to_path(&env, dir.path());
    let after = std::env::var_os("PATH").expect("PATH should exist after prepend");
    let paths: Vec<_> = std::env::split_paths(&after).collect();
    assert_eq!(paths, vec![dir.path().to_path_buf()]);
    drop(guard);
    assert!(std::env::var_os("PATH").is_none());
    {
        let _lock = EnvLock::acquire();
        if let Some(path) = original {
            unsafe { std::env::set_var("PATH", path) };
        } else {
            unsafe { std::env::remove_var("PATH") };
        }
    }
}
