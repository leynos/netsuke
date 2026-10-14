//! Integration tests for the `which` filter/function covering PATH resolution,
//! canonicalization, cwd behaviour, workspace fallback, and diagnostic output.

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::context;
use netsuke::stdlib::StdlibConfig;
use rstest::rstest;
use test_support::fs;

use super::support::{self, fallible};
use super::which_filter_common::*;

fn render_and_assert_pure(fixture: &mut WhichTestFixture, template: &Template) -> Result<String> {
    fixture.state.reset_impure();
    let output = fixture.render(template)?;
    ensure!(
        !fixture.state.is_impure(),
        "which lookup should leave the template pure"
    );
    Ok(output)
}

fn test_cache_after_removal(
    fixture: &mut WhichTestFixture,
    first_template: &Template,
    second_template: &Template,
    expect_second_err: bool,
) -> Result<()> {
    fixture.state.reset_impure();
    let first = fixture.render(first_template)?;
    let removed_path = fixture
        .paths
        .first()
        .context("cache-removal fixture should contain a tool path")?
        .clone();
    let expected_path = expected_which_output_path(&removed_path);
    ensure!(
        first == expected_path,
        "initial which lookup should resolve {expected_path}, got {first}"
    );

    fs::remove_file(&removed_path)?;

    fixture.state.reset_impure();
    let second_result = fixture.render(second_template);

    if expect_second_err {
        let err = second_result
            .err()
            .context("expected fresh which lookup to fail after removal")?;
        ensure!(
            err.to_string().contains("not_found"),
            "fresh lookup error should report not_found: {err}"
        );
    } else {
        let second = second_result?;
        ensure!(
            second == expected_path,
            "cached lookup should preserve {expected_path}, got {second}"
        );
    }

    Ok(())
}

/// Helper to set up a fixture for cache removal tests and run the test.
fn test_cache_behaviour_after_removal(
    second_template_str: &str,
    expect_second_err: bool,
) -> Result<()> {
    let mut fixture =
        WhichTestFixture::with_tool_in_dirs(&ToolName::from("helper"), &[DirName::from("bin")])?;

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
    let fixture_path = fixture
        .paths
        .first()
        .context("duplicate-path fixture should contain a tool path")?;
    let expected_output_path = if canonical {
        expected_canonical_which_output_path(fixture_path)?
    } else {
        expected_which_output_path(fixture_path)
    };

    ensure!(
        parts.len() == expected_count,
        "expected {expected_count} which results, got {}",
        parts.len()
    );
    for part in &parts {
        ensure!(
            *part == expected_output_path,
            "expected duplicate path {expected_output_path}, got {part}"
        );
    }

    Ok(())
}

/// Helper to test `cwd_mode` resolution with a given case variant.
fn test_cwd_mode_resolution(cwd_mode_value: &str) -> Result<()> {
    let (_temp, root) = support::filter_workspace()?;
    let tool = write_tool(&root, &ToolName::from("local"))?;
    let path = PathEnv::new(&[])?;
    let config = StdlibConfig::new(
        Dir::open_ambient_dir(&root, ambient_authority()).context("open workspace")?,
    )?
    .with_workspace_root_path(root)?;
    let (mut env, _state) =
        fallible::stdlib_env_with_config(config.with_path_override(path.into_inner()))?;
    let template = Template::new(format!(
        "{{{{ which('local', cwd_mode='{cwd_mode_value}') }}}}"
    ));
    let output = render(&mut env, &template)?;
    let expected_path = expected_which_output_path(&tool);
    ensure!(
        output == expected_path,
        "cwd_mode {cwd_mode_value} should resolve {expected_path}, got {output}"
    );
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
fn which_filter_all_with_duplicates_respects_canonical_false() -> Result<()> {
    let mut fixture = WhichTestFixture::with_tool_in_dirs(
        &ToolName::from("helper"),
        &[DirName::from("bin"), DirName::from("bin")],
    )?;
    test_duplicate_paths(&mut fixture, false, 2)
}

#[rstest]
fn which_filter_all_with_duplicates_deduplicates_canonicalized_paths() -> Result<()> {
    let mut fixture = WhichTestFixture::with_tool_in_dirs(
        &ToolName::from("helper"),
        &[DirName::from("bin"), DirName::from("bin")],
    )?;
    test_duplicate_paths(&mut fixture, true, 1)
}

#[rstest]
fn which_function_rejects_invalid_cwd_mode() -> Result<()> {
    let (_temp, _root) = support::filter_workspace()?;
    let path = PathEnv::new(&[])?;
    let (mut env, _state) = fallible::stdlib_env_with_path(path.into_inner())?;
    let template = Template::from("{{ which('local', cwd_mode='invalid') }}");

    let err = render(&mut env, &template)
        .err()
        .context("expected invalid cwd_mode to fail")?;

    let message = err.to_string();
    ensure!(
        message.contains("netsuke::jinja::which::args"),
        "expected which args error, got: {message}",
    );
    ensure!(
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
fn which_filter_skips_heavy_directories() -> Result<()> {
    let (_temp, root) = support::filter_workspace()?;
    let target = root.join("target");
    fs::create_dir_all(&target)?;
    write_tool(&target, &ToolName::from("helper"))?;
    // Root the resolver at the fixture workspace; otherwise the lookup searches
    // the process working directory, `target/helper` is never a candidate, and
    // the `not_found` assertion below would hold regardless of the skip policy.
    let config = StdlibConfig::new(
        Dir::open_ambient_dir(&root, ambient_authority()).context("open workspace")?,
    )?
    .with_workspace_root_path(root)?;
    let path = PathEnv::new(&[])?;
    let (env, _state) =
        fallible::stdlib_env_with_config(config.with_path_override(path.into_inner()))?;
    let err = env
        .render_str(
            "{{ 'helper' | which(cwd_mode='workspace-recursive') }}",
            context! {},
        )
        .err()
        .context("render should fail when tool is in skipped directory")?;
    ensure!(
        err.to_string().contains("not_found"),
        "skipped-directory lookup should report not_found: {err}"
    );
    Ok(())
}

#[rstest]
fn which_resolver_honours_workspace_root_override() -> Result<()> {
    let (_temp, root) = support::filter_workspace()?;
    let tool = write_tool(&root, &ToolName::from("helper"))?;
    let config = StdlibConfig::new(
        Dir::open_ambient_dir(&root, ambient_authority()).context("open workspace")?,
    )?
    .with_workspace_root_path(root.clone())?;
    let path = PathEnv::new(&[])?;
    let (mut env, _state) =
        fallible::stdlib_env_with_config(config.with_path_override(path.into_inner()))?;
    let output = render(
        &mut env,
        &Template::from("{{ 'helper' | which(cwd_mode='workspace-recursive') }}"),
    )?;
    let expected_path = expected_which_output_path(&tool);
    ensure!(
        output == expected_path,
        "workspace override should resolve {expected_path}, got {output}"
    );
    Ok(())
}
