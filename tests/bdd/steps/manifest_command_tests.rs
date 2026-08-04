//! Unit tests for `manifest_command` environment-handling behaviour.
//!
//! These tests verify that `build_netsuke_command` correctly isolates the
//! child-process environment: forwarding only scenario-tracked vars from
//! `TestWorld.env_vars_forward` and the host `PATH`, while stripping all
//! other inherited variables via `env_clear()`.

use super::*;
use anyhow::ensure;
use mockable::{DefaultEnv, Env};
use rstest::fixture;
use std::ffi::{OsStr, OsString};

fn env_value<'a>(cmd: &'a assert_cmd::Command, key: &str) -> Option<&'a OsStr> {
    cmd.get_envs()
        .find(|(k, _)| *k == OsStr::new(key))
        .and_then(|(_, v)| v)
}

#[fixture]
fn prepared_world() -> Result<TestWorld> {
    let world = TestWorld::default();
    let temp = tempfile::tempdir().context("create temp dir")?;
    *world.temp_dir.borrow_mut() = Some(temp);
    Ok(world)
}

#[rstest::rstest]
fn world_env_vars_with_value_are_applied(prepared_world: Result<TestWorld>) -> Result<()> {
    let world = prepared_world?;

    // Track the env var in TestWorld's forward map - this is the value that
    // will be forwarded to the child command, not read from process env.
    world.track_env_var(
        "NETSUKE_TEST_FLAG".to_owned(),
        Some(OsString::from("enabled")),
    );

    let cmd = build_netsuke_command(&world, &["--help"]).expect("build command");

    let val = env_value(&cmd, "NETSUKE_TEST_FLAG").expect("NETSUKE_TEST_FLAG should be present");
    ensure!(
        val == OsStr::new("enabled"),
        "expected NETSUKE_TEST_FLAG to be 'enabled', got {:?}",
        val
    );
    Ok(())
}

#[rstest::rstest]
fn host_env_vars_are_not_inherited(prepared_world: Result<TestWorld>) -> Result<()> {
    let world = prepared_world?;

    let cmd = build_netsuke_command(&world, &["--help"]).expect("build command");

    // Command should NOT contain the host env var because env_clear() was called
    let val = env_value(&cmd, "NETSUKE_HOST_VAR");
    ensure!(
        val.is_none(),
        "NETSUKE_HOST_VAR should not be inherited from host environment"
    );
    Ok(())
}

#[rstest::rstest]
fn scenario_path_override_is_forwarded_and_netsuke_executable_is_used(
    prepared_world: Result<TestWorld>,
) -> Result<()> {
    let world = prepared_world?;

    world.track_env_var("PATH".to_owned(), Some(OsString::from("/fake/bin")));

    let cmd = build_netsuke_command(&world, &["--version"]).expect("build command");

    // The scenario-provided PATH is forwarded explicitly after env_clear().
    let path_val =
        env_value(&cmd, "PATH").expect("PATH should be explicitly forwarded to the command");
    ensure!(
        path_val == OsStr::new("/fake/bin"),
        "expected PATH to be '/fake/bin', got {:?}",
        path_val
    );

    // Command should use the resolved netsuke_executable(), not rely on PATH lookup.
    let exe = netsuke_executable().expect("netsuke_executable");
    ensure!(
        cmd.get_program() == exe.as_os_str(),
        "expected program to be {:?}, got {:?}",
        exe,
        cmd.get_program()
    );
    Ok(())
}

#[rstest::rstest]
fn host_path_is_forwarded_when_the_scenario_has_no_override(
    prepared_world: Result<TestWorld>,
) -> Result<()> {
    let world = prepared_world?;
    let expected = DefaultEnv
        .os_string("PATH")
        .expect("the test host should provide PATH");

    let cmd = build_netsuke_command(&world, &["--version"]).expect("build command");

    let actual = env_value(&cmd, "PATH").expect("host PATH should be forwarded after env_clear");
    ensure!(
        actual == expected,
        "expected host PATH {expected:?}, got {actual:?}"
    );
    Ok(())
}
