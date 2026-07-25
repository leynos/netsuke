//! Verbose Ninja command logging and override property tests.

use super::support::{
    fake_ninja_name, path_containing, run_verbose_build_with_ninja_env, temp_with_minimal_manifest,
    write_fake_ninja_script,
};
use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};
use proptest::prelude::*;
use rstest::rstest;
use std::path::Path;
use tempfile::{TempDir, tempdir};

fn run_verbose_build_and_assert_ninja_log(
    temp_with_minimal_manifest: Result<TempDir>,
    ninja_stem: &str,
    override_mode: NinjaOverride,
) -> Result<()> {
    let temp = temp_with_minimal_manifest?;
    let description = match override_mode {
        NinjaOverride::Unset => "default build should log the fallback ninja program",
        NinjaOverride::FullPath => "full-path override should log the resolved ninja program",
        NinjaOverride::Name => "named override should resolve the ninja program through PATH",
    };
    run_verbose_build_with_fake_ninja_and_assert_log(
        temp.path(),
        ninja_stem,
        override_mode,
        description,
    )
}

#[derive(Clone, Copy)]
enum NinjaOverride {
    Unset,
    FullPath,
    Name,
}

fn run_verbose_build_with_fake_ninja_and_assert_log(
    workspace: &Path,
    ninja_stem: &str,
    override_mode: NinjaOverride,
    description: &str,
) -> Result<()> {
    let ninja_temp = tempdir().context("create fake ninja dir")?;
    let ninja_dir_path =
        Utf8Path::from_path(ninja_temp.path()).context("fake ninja directory path is not UTF-8")?;
    let ninja_dir = Dir::open_ambient_dir(ninja_dir_path, ambient_authority())
        .context("open fake ninja directory")?;
    let ninja_name = fake_ninja_name(ninja_stem);
    write_fake_ninja_script(&ninja_dir, Utf8Path::new(&ninja_name), &[], None)?;
    let ninja_path = ninja_temp.path().join(&ninja_name);

    let ninja_env = match override_mode {
        NinjaOverride::Unset => None,
        NinjaOverride::FullPath => Some(ninja_path.as_path()),
        NinjaOverride::Name => Some(Path::new(&ninja_name)),
    };
    let stderr = run_verbose_build_with_ninja_env(
        workspace,
        path_containing(ninja_temp.path())?,
        ninja_env,
    )?;

    let expected = match override_mode {
        NinjaOverride::Unset | NinjaOverride::Name => format!("Executing command: {ninja_stem} "),
        NinjaOverride::FullPath => format!("Executing command: {} ", ninja_path.display()),
    };
    ensure!(stderr.contains(&expected), "{description}, got:\n{stderr}");
    Ok(())
}

fn assert_verbose_build_logs_ninja_override(stem: &str) -> Result<()> {
    let temp = temp_with_minimal_manifest()?;
    run_verbose_build_with_fake_ninja_and_assert_log(
        temp.path(),
        stem,
        NinjaOverride::FullPath,
        &format!("verbose log must contain the resolved ninja path for override stem {stem:?}"),
    )
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 16, ..ProptestConfig::default() })]
    #[test]
    fn verbose_build_logs_resolved_ninja_program_for_any_valid_override(
        stem in "[a-z][a-z0-9_-]{0,15}",
    ) {
        assert_verbose_build_logs_ninja_override(&stem)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
    }
}

#[rstest]
fn verbose_build_logs_default_ninja_command(
    temp_with_minimal_manifest: Result<TempDir>,
) -> Result<()> {
    run_verbose_build_and_assert_ninja_log(
        temp_with_minimal_manifest,
        "ninja",
        NinjaOverride::Unset,
    )
}

#[rstest]
#[case::full_path(NinjaOverride::FullPath)]
#[case::path_lookup(NinjaOverride::Name)]
fn verbose_build_logs_ninja_env_override(
    temp_with_minimal_manifest: Result<TempDir>,
    #[case] override_mode: NinjaOverride,
) -> Result<()> {
    run_verbose_build_and_assert_ninja_log(
        temp_with_minimal_manifest,
        "custom-ninja",
        override_mode,
    )
}
