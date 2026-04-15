//! Tests for `restore_many_locked` and `restore_many` in `test_support::env`.
//!
//! These functions restore batches of environment variables under the global
//! `EnvLock`. Tests verify that set variables are restored, absent variables
//! are removed, and empty snapshots are handled gracefully.

use anyhow::{Result, ensure};
use rstest::rstest;
use std::collections::HashMap;
use std::ffi::OsString;
use test_support::env::{remove_var, restore_many, set_var};

/// Return a unique variable name scoped to a test to avoid collisions.
fn test_var(suffix: &str) -> String {
    format!("_NETSUKE_RESTORE_TEST_{suffix}")
}

#[rstest]
fn restore_many_restores_previously_set_variable() -> Result<()> {
    let key = test_var("SET");
    let _previous = set_var(&key, std::ffi::OsStr::new("original"));

    let mut snapshot = HashMap::new();
    snapshot.insert(key.clone(), Some(OsString::from("original")));

    // Overwrite then restore
    let _ = set_var(&key, std::ffi::OsStr::new("changed"));
    ensure!(
        std::env::var(&key).expect("key should exist") == "changed",
        "precondition: variable should be overwritten"
    );

    restore_many(snapshot);

    ensure!(
        std::env::var(&key).expect("key should exist") == "original",
        "variable should be restored to original value"
    );

    remove_var(&key);
    Ok(())
}

#[rstest]
fn restore_many_removes_variable_when_prior_value_is_none() -> Result<()> {
    let key = test_var("NONE");
    let _ = set_var(&key, std::ffi::OsStr::new("transient"));

    let mut snapshot = HashMap::new();
    snapshot.insert(key.clone(), None);

    restore_many(snapshot);

    ensure!(
        std::env::var_os(&key).is_none(),
        "variable should be removed when prior value was None"
    );
    Ok(())
}

#[test]
fn restore_many_handles_empty_map() {
    restore_many(HashMap::new());
    // No-op — should not panic.
}

#[rstest]
fn restore_many_restores_multiple_variables() -> Result<()> {
    let key_a = test_var("MULTI_A");
    let key_b = test_var("MULTI_B");
    let _ = set_var(&key_a, std::ffi::OsStr::new("a_orig"));
    let _ = set_var(&key_b, std::ffi::OsStr::new("b_orig"));

    let mut snapshot = HashMap::new();
    snapshot.insert(key_a.clone(), Some(OsString::from("a_orig")));
    snapshot.insert(key_b.clone(), Some(OsString::from("b_orig")));

    // Overwrite both
    let _ = set_var(&key_a, std::ffi::OsStr::new("a_changed"));
    let _ = set_var(&key_b, std::ffi::OsStr::new("b_changed"));

    restore_many(snapshot);

    ensure!(
        std::env::var(&key_a).expect("key_a") == "a_orig",
        "first variable should be restored"
    );
    ensure!(
        std::env::var(&key_b).expect("key_b") == "b_orig",
        "second variable should be restored"
    );

    remove_var(&key_a);
    remove_var(&key_b);
    Ok(())
}

#[rstest]
fn restore_many_mixed_set_and_remove() -> Result<()> {
    let key_set = test_var("MIX_SET");
    let key_remove = test_var("MIX_REMOVE");

    // key_set originally had a value; key_remove did not exist
    let _ = set_var(&key_set, std::ffi::OsStr::new("keep_me"));
    remove_var(&key_remove);

    let mut snapshot = HashMap::new();
    snapshot.insert(key_set.clone(), Some(OsString::from("keep_me")));
    snapshot.insert(key_remove.clone(), None);

    // Overwrite: change key_set, create key_remove
    let _ = set_var(&key_set, std::ffi::OsStr::new("changed"));
    let _ = set_var(&key_remove, std::ffi::OsStr::new("should_vanish"));

    restore_many(snapshot);

    ensure!(
        std::env::var(&key_set).expect("key_set") == "keep_me",
        "set variable should be restored to original"
    );
    ensure!(
        std::env::var_os(&key_remove).is_none(),
        "variable that did not exist should be removed"
    );

    remove_var(&key_set);
    Ok(())
}
