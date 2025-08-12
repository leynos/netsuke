use mockable::Env;
use rstest::rstest;
use serial_test::serial;

#[path = "support/env.rs"]
mod env;
mod support;
use env::{mocked_path_env, prepend_dir_to_path};

#[rstest]
#[serial]
fn prepend_dir_to_path_sets_and_restores() {
    let env = mocked_path_env();
    let original = env.raw("PATH").expect("PATH missing in mock");
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
