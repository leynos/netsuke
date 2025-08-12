//! Tests for PATH restoration behaviour using mock environments.
//!
//! Verifies that `PathGuard` restores `PATH` without mutating the real
//! process environment.

#[path = "support/env_lock.rs"]
mod env_lock;
#[path = "support/path_guard.rs"]
mod path_guard;

use mockall::{Sequence, mock};
use path_guard::{Env, PathGuard};
use std::ffi::OsStr;

mock! {
    pub Env {}
    impl Env for Env {
        unsafe fn set_var(&mut self, key: &str, val: &OsStr);
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
        let mut guard = PathGuard::with_env("/orig".into(), env);
        unsafe {
            guard.env_mut().set_var("PATH", OsStr::new("/tmp"));
        }
    }
}
