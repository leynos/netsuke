//! Integration tests for the `which` filter/function covering PATH resolution,
//! canonicalisation, cwd behaviour, workspace fallback, and diagnostic output.

use anyhow::{Context, Result};
use netsuke::stdlib::StdlibConfig;
use rstest::rstest;
use serde_json::Value;
use std::{env, path::PathBuf};
use tempfile::tempdir;
use test_support::env_lock::EnvLock;

use super::support::{self, fallible};
use super::which_filter_common::*;

struct CurrentDirGuard {
    original: PathBuf,
    _lock: EnvLock,
}

impl CurrentDirGuard {
    fn change_to(path: &std::path::Path) -> Result<Self> {
        let lock = EnvLock::acquire();
        let original = env::current_dir().context("capture current working directory")?;
        env::set_current_dir(path).context("switch cwd")?;
        Ok(Self {
            original,
            _lock: lock,
        })
    }
}

impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        if let Err(err) = env::set_current_dir(&self.original) {
            tracing::warn!(
                "failed to restore working directory to {}: {err}",
                self.original.display()
            );
        }
    }
}

fn render_and_assert_pure(
    fixture: &mut WhichTestFixture,
    template: &Template,
) -> Result<String> {
    fixture.state.reset_impure();
    let output = fixture.render(template)?;
    assert!(!fixture.state.is_impure());
    Ok(output)
}

fn test_cache_after_removal(
    fixture: &mut WhichTestFixture,
    first_template: &Template,
    second_template: &Template,
    expect_second_err: bool,
) -> Result<()> {
    fixture.state.reset_impure();
    let removed_path = &fixture.paths[0];
    let first = fixture.render(first_template)?;
    assert_eq!(first, removed_path.as_str());

    std::fs::remove_file(removed_path)?;

    fixture.state.reset_impure();
    let second_result = fixture.render(second_template);

    if expect_second_err {
        let err = second_result.expect_err("expected fresh which lookup to fail after removal");
        assert!(err.to_string().contains("not_found"));
    } else {
        let second = second_result?;
        assert_eq!(second, removed_path.as_str());
    }

    Ok(())
}

/// Helper to set up a fixture for cache removal tests and run the test.
fn test_cache_behaviour_after_removal(
    second_template_str: &str,
    expect_second_err: bool,
) -> Result<()> {
    let mut fixture = WhichTestFixture::with_tool_in_dirs(
        &ToolName::from("helper"),
        &[DirName::from("bin_first"), DirName::from("bin_second")],
    )?;

    test_cache_after_removal(
        &mut fixture,
        &Template::from("{{ 'helper' | which }}"),
        &Template::from(second_template_str),
        expect_second_err,
    )
}

fn test_duplicate_paths(
    fixture: &mut WhichTestFixture,
    canonical: bool,
    expected_count: usize,
) -> Result<()> {
    let template = if canonical {
        Template::from("{{ 'helper' | which(all=true, canonical=true) | join('|') }}")
    } else {
        Template::from("{{ 'helper' | which(all=true, canonical=false) | join('|') }}")
    };

    let output = render_and_assert_pure(fixture, &template)?;
    let parts: Vec<&str> = output.split('|').collect();

    assert_eq!(parts.len(), expected_count);
    for part in &parts {
        assert_eq!(*part, fixture.paths[0].as_str());
    }

    Ok(())
}

/// Helper to test cwd_mode resolution with a given case variant.
fn test_cwd_mode_resolution(cwd_mode_value: &str) -> Result<()> {
    let (_temp, root) = support::filter_workspace()?;
    let tool = write_tool(&root, &ToolName::from("local"))?;
    let _path = PathEnv::new(&[])?;
    let (mut env, _state) = fallible::stdlib_env_with_state()?;
    let template = Template::from(format!("{{{{ which('local', cwd_mode='{cwd_mode_value}') }}}}"));
    let output = render(&mut env, &template)?;
    assert_eq!(output, tool.as_str());
    Ok(())
}

#[rstest]
fn which_filter_returns_first_match() -> Result<()> {
    let mut fixture = WhichTestFixture::with_tool_in_dirs(
        &ToolName::from("helper"),
        &[DirName::from("bin_first"), DirName::from("bin_second")],
    )?;
    let output = render_and_assert_pure(&mut fixture, &Template::from("{{ 'helper' | which }}"))?;
    assert_eq!(output, fixture.paths[0].as_str());
    Ok(())
}

#[rstest]
fn which_filter_uses_cached_result_when_executable_removed() -> Result<()> {
    test_cache_behaviour_after_removal("{{ 'helper' | which }}", false)
}

#[rstest]
fn which_filter_fresh_bypasses_cache_after_executable_removed() -> Result<()> {
    test_cache_behaviour_after_removal("{{ 'helper' | which(fresh=true) }}", true)
}

#[rstest]
fn which_filter_all_returns_all_matches() -> Result<()> {
    let mut fixture = WhichTestFixture::with_tool_in_dirs(
        &ToolName::from("helper"),
        &[DirName::from("bin_a"), DirName::from("bin_b")],
    )?;
    let output = render_and_assert_pure(
        &mut fixture,
        &Template::from("{{ 'helper' | which(all=true) | join('|') }}"),
    )?;
    let expected = format!(
        "{}|{}",
        fixture.paths[0].as_str(),
        fixture.paths[1].as_str()
    );
    assert_eq!(output, expected);
    Ok(())
}

#[rstest]
fn which_filter_all_returns_list() -> Result<()> {
    let mut fixture = WhichTestFixture::with_tool_in_dirs(
        &ToolName::from("helper"),
        &[DirName::from("bin_a"), DirName::from("bin_b")],
    )?;

    let output = fixture.render(&Template::from("{{ which('helper', all=true) | tojson }}"))?;
    let value: Value = serde_json::from_str(&output)?;
    assert!(
        value.is_array(),
        "expected which(..., all=true) to return a JSON array, got {value:?}",
    );

    Ok(())
}

#[rstest]
fn which_filter_all_with_duplicates_respects_canonical_false() -> Result<()> {
    let mut fixture = WhichTestFixture::with_tool_in_dirs(
        &ToolName::from("helper"),
        &[DirName::from("bin"), DirName::from("bin")],
    )?;
    test_duplicate_paths(&mut fixture, false, 2)
}

#[rstest]
fn which_filter_all_with_duplicates_deduplicates_canonicalised_paths() -> Result<()> {
    let mut fixture = WhichTestFixture::with_tool_in_dirs(
        &ToolName::from("helper"),
        &[DirName::from("bin"), DirName::from("bin")],
    )?;
    test_duplicate_paths(&mut fixture, true, 1)
}

#[rstest]
fn which_function_honours_cwd_mode() -> Result<()> {
    test_cwd_mode_resolution("always")
}

#[rstest]
fn which_function_rejects_invalid_cwd_mode() -> Result<()> {
    let (_temp, _root) = support::filter_workspace()?;
    let _path = PathEnv::new(&[])?;
    let (mut env, _state) = fallible::stdlib_env_with_state()?;
    let template = Template::from("{{ which('local', cwd_mode='invalid') }}");

    let err = render(&mut env, &template)
        .expect_err("expected invalid cwd_mode to fail");

    let message = err.to_string();
    assert!(
        message.contains("netsuke::jinja::which::args"),
        "expected which args error, got: {message}",
    );
    assert!(
        message.contains("cwd_mode"),
        "expected message to mention cwd_mode, got: {message}",
    );

    Ok(())
}

#[rstest]
fn which_function_accepts_case_insensitive_cwd_mode() -> Result<()> {
    test_cwd_mode_resolution("ALWAYS")
}

#[rstest]
fn which_filter_reports_missing_command() -> Result<()> {
    let (_temp, _root) = support::filter_workspace()?;
    let _path = PathEnv::new(&[])?;
    let (mut env, _state) = fallible::stdlib_env_with_state()?;
    let err = env
        .render_str("{{ 'absent' | which }}", context! {})
        .unwrap_err();
    let message = err.to_string();
    assert!(message.contains("netsuke::jinja::which::not_found"));
    Ok(())
}

#[rstest]
fn which_filter_falls_back_to_workspace_when_path_empty() -> Result<()> {
    let (_temp, root) = support::filter_workspace()?;
    let tool = write_tool(&root, &ToolName::from("helper"))?;
    let _path = PathEnv::new(&[])?;
    let (mut env, _state) = fallible::stdlib_env_with_state()?;
    let output = render(&mut env, &Template::from("{{ 'helper' | which }}"))?;
    assert_eq!(output, tool.as_str());
    Ok(())
}

#[rstest]
fn which_filter_skips_heavy_directories() -> Result<()> {
    let (_temp, root) = support::filter_workspace()?;
    let target = root.join("target");
    std::fs::create_dir_all(target.as_std_path())?;
    write_tool(&target, &ToolName::from("helper"))?;
    let _path = PathEnv::new(&[])?;
    let (mut env, _state) = fallible::stdlib_env_with_state()?;
    let err = env
        .render_str("{{ 'helper' | which }}", context! {})
        .unwrap_err();
    assert!(err.to_string().contains("not_found"));
    Ok(())
}

#[rstest]
fn which_resolver_honours_workspace_root_override() -> Result<()> {
    use cap_std::{ambient_authority, fs_utf8::Dir};
    let (_temp, root) = support::filter_workspace()?;
    let tool = write_tool(&root, &ToolName::from("helper"))?;
    let alt = tempdir().context("create alternate cwd")?;
    let _cwd_guard = CurrentDirGuard::change_to(alt.path())?;

    let config = StdlibConfig::new(
        Dir::open_ambient_dir(&root, ambient_authority()).context("open workspace")?,
    )?
    .with_workspace_root_path(root.clone())?;
    let _path = PathEnv::new(&[])?;
    let (mut env, _state) = fallible::stdlib_env_with_config(config)?;
    let output = render(&mut env, &Template::from("{{ 'helper' | which }}"))?;
    assert_eq!(output, tool.as_str());
    Ok(())
}
