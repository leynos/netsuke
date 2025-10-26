//! Path filter tests for the standard filter library.
//!
//! Tests for path manipulation filters including dirname, relative_to,
//! with_suffix, realpath, and expanduser. Each test validates filter
//! behaviour with various inputs and error conditions.

use anyhow::{anyhow, bail, ensure, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{Environment, ErrorKind};
use rstest::rstest;
use serde_json::{Value, json};

use super::support::{
    EnvLock, EnvVarGuard, Workspace, fallible,
};

/// Helper for tests requiring environment variable manipulation
fn with_clean_env_vars<F, R>(home_value: Option<&str>, test_fn: F) -> R
where
    F: FnOnce() -> R,
{
    let _lock = EnvLock::acquire();
    let _home = home_value.map_or_else(
        || EnvVarGuard::remove("HOME"),
        |value| EnvVarGuard::set("HOME", value),
    );
    let _profile = EnvVarGuard::remove("USERPROFILE");
    let _drive = EnvVarGuard::remove("HOMEDRIVE");
    let _path = EnvVarGuard::remove("HOMEPATH");
    let _share = EnvVarGuard::remove("HOMESHARE");
    test_fn()
}

/// Helper for standard filter environment setup
fn setup_filter_env() -> Result<Environment<'static>> {
    fallible::stdlib_env()
}

fn with_filter_env<F>(workspace: Workspace, test_fn: F) -> Result<()>
where
    F: FnOnce(&Utf8Path, &mut Environment<'static>) -> Result<()>,
{
    let (_temp, root) = workspace;
    let mut env = setup_filter_env()?;
    test_fn(&root, &mut env)
}

struct TemplateErrorExpectation<'a> {
    kind: ErrorKind,
    contains: &'a str,
}

struct TemplateErrorSpec<'a> {
    name: &'a str,
    template: &'a str,
    context: Value,
    expectation: TemplateErrorExpectation<'a>,
}

/// Specification for a filter success test case.
struct FilterSuccessSpec<'a> {
    /// Template name for registration.
    name: &'static str,
    /// Template source code.
    template: &'static str,
    /// Path value to pass to the template.
    path: &'a Utf8PathBuf,
}

/// Helper for error testing with custom template
fn assert_template_error(env: &mut Environment<'_>, spec: TemplateErrorSpec<'_>) -> Result<()> {
    fallible::register_template(env, spec.name, spec.template)?;
    let template = env
        .get_template(spec.name)
        .with_context(|| format!("fetch template '{}'", spec.name))?;
    let TemplateErrorSpec {
        context,
        expectation,
        name,
        ..
    } = spec;
    let TemplateErrorExpectation { kind, contains } = expectation;
    let err = match template.render(context) {
        Ok(output) => bail!(
            "expected template '{name}' to fail but rendered {output}"
        ),
        Err(err) => err,
    };
    ensure!(
        err.kind() == kind,
        "template '{name}' should report {kind:?}, but was {:?}",
        err.kind()
    );
    ensure!(
        err.to_string().contains(contains),
        "error should mention '{contains}' but was: {err}"
    );
    Ok(())
}

fn assert_filter_error_with_env<F>(
    filter_workspace: Workspace,
    home_value: Option<&str>,
    spec_builder: F,
) -> Result<()>
where
    F: for<'a> FnOnce(&'a Utf8Path) -> TemplateErrorSpec<'a>,
{
    with_filter_env(filter_workspace, |root, env| {
        let home = home_value.map(|value| {
            if value.is_empty() {
                root.as_str()
            } else {
                value
            }
        });
        with_clean_env_vars(home, || {
            let spec = spec_builder(root);
            assert_template_error(env, spec)
        })
    })
}

fn assert_filter_error_simple<F>(filter_workspace: Workspace, spec_builder: F) -> Result<()>
where
    F: for<'a> FnOnce(&'a Utf8Path) -> TemplateErrorSpec<'a>,
{
    with_filter_env(filter_workspace, |root, env| {
        let spec = spec_builder(root);
        assert_template_error(env, spec)
    })
}

fn assert_filter_success_with_env<F>(
    filter_workspace: Workspace,
    home_value: Option<&str>,
    spec: FilterSuccessSpec<'_>,
    expected: F,
) -> Result<()>
where
    F: FnOnce(&Utf8Path) -> String,
{
    with_filter_env(filter_workspace, |root, env| {
        let home = home_value.map(|value| {
            if value.is_empty() {
                root.as_str()
            } else {
                value
            }
        });
        with_clean_env_vars(home, move || {
            let FilterSuccessSpec { name, template, path } = spec;
            let result = fallible::render(env, name, template, path)?;
            let expected_value = expected(root);
            ensure!(
                result == expected_value,
                "expected '{expected_value}' but rendered {result}"
            );
            Ok(())
        })
    })
}

/// Test data for filter error tests
struct FilterErrorTest {
    name: &'static str,
    template: &'static str,
    context: Value,
    error_kind: ErrorKind,
    error_contains: &'static str,
    env_setup: Option<EnvironmentSetup>,
}

#[derive(Debug, Clone, Copy)]
enum EnvironmentSetup {
    SetHome,
    RemoveHome,
}

/// Unified helper for all filter error tests
fn test_filter_error(filter_workspace: Workspace, test: FilterErrorTest) -> Result<()> {
    let FilterErrorTest {
        name,
        template,
        context,
        error_kind,
        error_contains,
        env_setup,
    } = test;

    if let Some(EnvironmentSetup::SetHome) = env_setup {
        return assert_filter_error_with_env(filter_workspace, Some(""), move |_root| TemplateErrorSpec {
            name,
            template,
            context: context.clone(),
            expectation: TemplateErrorExpectation {
                kind: error_kind,
                contains: error_contains,
            },
        });
    }

    if let Some(EnvironmentSetup::RemoveHome) = env_setup {
        return assert_filter_error_with_env(filter_workspace, None, move |_root| TemplateErrorSpec {
            name,
            template,
            context: context.clone(),
            expectation: TemplateErrorExpectation {
                kind: error_kind,
                contains: error_contains,
            },
        });
    }

    assert_filter_error_simple(filter_workspace, move |_root| TemplateErrorSpec {
        name,
        template,
        context,
        expectation: TemplateErrorExpectation {
            kind: error_kind,
            contains: error_contains,
        },
    })
}

#[rstest]
fn dirname_filter() -> Result<()> {
    let workspace = fallible::filter_workspace()?;
    with_filter_env(workspace, |root, env| {
        let file = root.join("file");
        let output = fallible::render(env, "dirname", "{{ path | dirname }}", &file)
            .context("render dirname filter")?;
        ensure!(
            output == root.as_str(),
            "expected dirname to yield workspace root, but rendered {output}"
        );
        Ok(())
    })?;
    Ok(())
}

#[rstest]
fn relative_to_filter() -> Result<()> {
    let workspace = fallible::filter_workspace()?;
    with_filter_env(workspace, |root, env| {
        let dir = Dir::open_ambient_dir(root, ambient_authority())
            .context("open workspace root")?;
        dir.create_dir_all("nested")
            .context("create nested directory")?;
        dir.write("nested/file.txt", b"data")
            .context("write nested file")?;
        let nested = root.join("nested/file.txt");
        let output = fallible::render(
            env,
            "relative_to",
            "{{ path | relative_to(path | dirname) }}",
            &nested,
        )
        .context("render relative_to filter")?;
        ensure!(output == "file.txt", "expected 'file.txt' but rendered {output}");
        Ok(())
    })?;
    Ok(())
}

#[rstest]
fn relative_to_filter_outside_root() -> Result<()> {
    let workspace = fallible::filter_workspace()?;
    test_filter_error(
        workspace,
        FilterErrorTest {
            name: "relative_to_fail",
            template: "{{ path | relative_to(root) }}",
            context: json!({
                "path": "/some/outside/path",
                "root": "workspace",
            }),
            error_kind: ErrorKind::InvalidOperation,
            error_contains: "is not relative",
            env_setup: None,
        },
    )
}

#[rstest]
fn with_suffix_filter() -> Result<()> {
    let workspace = fallible::filter_workspace()?;
    with_filter_env(workspace, |root, env| {
        let file = root.join("file.tar.gz");
        Dir::open_ambient_dir(root, ambient_authority())
            .context("open workspace root")?
            .write("file.tar.gz", b"data")
            .context("write archive fixture")?;
        let first = fallible::render(env, "suffix", "{{ path | with_suffix('.log') }}", &file)
            .context("render with_suffix(.log)")?;
        ensure!(
            first == root.join("file.tar.log").as_str(),
            "expected '.log' suffix to replace final component but rendered {first}"
        );
        let second = fallible::render(
            env,
            "suffix_alt",
            "{{ path | with_suffix('.zip', 2) }}",
            &file,
        )
        .context("render with_suffix(.zip, 2)")?;
        ensure!(
            second == root.join("file.zip").as_str(),
            "expected two extensions to be replaced but rendered {second}"
        );
        let third = fallible::render(
            env,
            "suffix_count_zero",
            "{{ path | with_suffix('.bak', 0) }}",
            &file,
        )
        .context("render with_suffix(.bak, 0)")?;
        ensure!(
            third == root.join("file.tar.gz.bak").as_str(),
            "expected zero count to append suffix but rendered {third}"
        );
        Ok(())
    })?;
    Ok(())
}

#[rstest]
fn with_suffix_filter_without_separator() -> Result<()> {
    let workspace = fallible::filter_workspace()?;
    with_filter_env(workspace, |root, env| {
        let file = root.join("file");
        let output = fallible::render(
            env,
            "suffix_plain",
            "{{ path | with_suffix('.log') }}",
            &file,
        )
        .context("render with_suffix on filename without separator")?;
        ensure!(
            output == root.join("file.log").as_str(),
            "expected '.log' to be appended but rendered {output}"
        );
        Ok(())
    })?;
    Ok(())
}

#[rstest]
fn with_suffix_filter_empty_separator() -> Result<()> {
    let workspace = fallible::filter_workspace()?;
    test_filter_error(
        workspace,
        FilterErrorTest {
            name: "suffix_empty_sep",
            template: "{{ path | with_suffix('.log', 1, '') }}",
            context: json!({
                "path": "file.tar.gz",
            }),
            error_kind: ErrorKind::InvalidOperation,
            error_contains: "non-empty separator",
            env_setup: None,
        },
    )
}

#[rstest]
fn with_suffix_filter_excessive_count() -> Result<()> {
    let workspace = fallible::filter_workspace()?;
    with_filter_env(workspace, |root, env| {
        let file = root.join("file.tar.gz");
        let output = fallible::render(
            env,
            "suffix_excessive",
            "{{ path | with_suffix('.bak', 5) }}",
            &file,
        )
        .context("render with_suffix(.bak, 5)")?;
        ensure!(
            output == root.join("file.bak").as_str(),
            "expected excessive count to collapse extensions but rendered {output}"
        );
        Ok(())
    })?;
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn realpath_filter() -> Result<()> {
    let workspace = fallible::filter_workspace()?;
    with_filter_env(workspace, |root, env| {
        let link = root.join("link");
        let output = fallible::render(env, "realpath", "{{ path | realpath }}", &link)
            .context("render realpath filter")?;
        ensure!(
            output == root.join("file").as_str(),
            "expected symlink to resolve to file but rendered {output}"
        );
        Ok(())
    })?;
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn realpath_filter_missing_path() -> Result<()> {
    let workspace = fallible::filter_workspace()?;
    test_filter_error(
        workspace,
        FilterErrorTest {
            name: "realpath_missing",
            template: "{{ path | realpath }}",
            context: json!({
                "path": "missing_file.txt",
            }),
            error_kind: ErrorKind::InvalidOperation,
            error_contains: "not found",
            env_setup: None,
        },
    )
}

#[cfg(unix)]
#[rstest]
fn realpath_filter_root_path() -> Result<()> {
    let workspace = fallible::filter_workspace()?;
    with_filter_env(workspace, |root, env| {
        let root_path = root
            .ancestors()
            .find(|candidate| candidate.parent().is_none())
            .map(Utf8Path::to_path_buf)
            .ok_or_else(|| anyhow!("unable to determine filesystem root"))?;
        ensure!(
            !root_path.as_str().is_empty(),
            "root path should not be empty"
        );
        let output = fallible::render(env, "realpath_root", "{{ path | realpath }}", &root_path)
            .context("render realpath for filesystem root")?;
        ensure!(
            output == root_path.as_str(),
            "expected filesystem root to resolve to itself but rendered {output}"
        );
        Ok(())
    })?;
    Ok(())
}

#[rstest]
fn expanduser_filter() -> Result<()> {
    let workspace = fallible::filter_workspace()?;
    let path = Utf8PathBuf::from("~/workspace");
    assert_filter_success_with_env(
        workspace,
        Some(""),
        FilterSuccessSpec {
            name: "expanduser",
            template: "{{ path | expanduser }}",
            path: &path,
        },
        |root| root.join("workspace").as_str().to_owned(),
    )
}

#[rstest]
fn expanduser_filter_non_tilde_path() -> Result<()> {
    let workspace = fallible::filter_workspace()?;
    with_filter_env(workspace, |root, env| {
        let file = root.join("file");
        let output = fallible::render(env, "expanduser_plain", "{{ path | expanduser }}", &file)
            .context("render expanduser on non-tilde path")?;
        ensure!(output == file.as_str(), "expected path to remain unchanged but rendered {output}");
        Ok(())
    })?;
    Ok(())
}

#[rstest]
fn expanduser_filter_missing_home() -> Result<()> {
    let workspace = fallible::filter_workspace()?;
    test_filter_error(
        workspace,
        FilterErrorTest {
            name: "expanduser_missing_home",
            template: "{{ path | expanduser }}",
            context: json!({
                "path": "~/workspace",
            }),
            error_kind: ErrorKind::InvalidOperation,
            error_contains: "no home directory environment variables are set",
            env_setup: Some(EnvironmentSetup::RemoveHome),
        },
    )
}

#[rstest]
fn expanduser_filter_user_specific() -> Result<()> {
    let workspace = fallible::filter_workspace()?;
    test_filter_error(
        workspace,
        FilterErrorTest {
            name: "expanduser_user_specific",
            template: "{{ path | expanduser }}",
            context: json!({
                "path": "~otheruser/workspace",
            }),
            error_kind: ErrorKind::InvalidOperation,
            error_contains: "user-specific ~ expansion is unsupported",
            env_setup: Some(EnvironmentSetup::SetHome),
        },
    )
}
