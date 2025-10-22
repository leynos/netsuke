#![allow(clippy::expect_used, reason = "path filter tests use expect for ease of debugging")]

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{Environment, ErrorKind};
use rstest::rstest;
use serde_json::{Value, json};

use super::support::{
    EnvLock, EnvVarGuard, Workspace, filter_workspace, register_template, render, stdlib_env,
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
fn setup_filter_env() -> Environment<'static> {
    stdlib_env()
}

fn with_filter_env<F>(workspace: Workspace, test_fn: F)
where
    F: FnOnce(&Utf8Path, &mut Environment<'static>),
{
    let (_temp, root) = workspace;
    let mut env = setup_filter_env();
    test_fn(&root, &mut env);
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

/// Helper for error testing with custom template
fn assert_template_error(env: &mut Environment<'_>, spec: TemplateErrorSpec<'_>) {
    register_template(env, spec.name, spec.template);
    let template = env.get_template(spec.name).expect("get template");
    let TemplateErrorSpec {
        context,
        expectation,
        ..
    } = spec;
    let result = template.render(context);
    let err = result.expect_err("template rendering should fail");
    assert_eq!(err.kind(), expectation.kind);
    assert!(
        err.to_string().contains(expectation.contains),
        "error should mention {}: {err}",
        expectation.contains
    );
}

fn assert_filter_error_with_env<F>(
    filter_workspace: Workspace,
    home_value: Option<&str>,
    spec_builder: F,
) where
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
            assert_template_error(env, spec);
        });
    });
}

fn assert_filter_error_simple<F>(filter_workspace: Workspace, spec_builder: F)
where
    F: for<'a> FnOnce(&'a Utf8Path) -> TemplateErrorSpec<'a>,
{
    with_filter_env(filter_workspace, |root, env| {
        let spec = spec_builder(root);
        assert_template_error(env, spec);
    });
}

fn assert_filter_success_with_env<F>(
    filter_workspace: Workspace,
    home_value: Option<&str>,
    name: &'static str,
    template: &'static str,
    path: &Utf8PathBuf,
    expected: F,
) where
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
        with_clean_env_vars(home, || {
            let result = render(env, name, template, path);
            let expected_value = expected(root);
            assert_eq!(result, expected_value);
        });
    });
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
fn test_filter_error(filter_workspace: Workspace, test: FilterErrorTest) {
    let FilterErrorTest {
        name,
        template,
        context,
        error_kind,
        error_contains,
        env_setup,
    } = test;

    if let Some(EnvironmentSetup::SetHome) = env_setup {
        assert_filter_error_with_env(filter_workspace, Some(""), move |_root| TemplateErrorSpec {
            name,
            template,
            context: context.clone(),
            expectation: TemplateErrorExpectation {
                kind: error_kind,
                contains: error_contains,
            },
        });
        return;
    }

    if let Some(EnvironmentSetup::RemoveHome) = env_setup {
        assert_filter_error_with_env(filter_workspace, None, move |_root| TemplateErrorSpec {
            name,
            template,
            context: context.clone(),
            expectation: TemplateErrorExpectation {
                kind: error_kind,
                contains: error_contains,
            },
        });
        return;
    }

    assert_filter_error_simple(filter_workspace, move |_root| TemplateErrorSpec {
        name,
        template,
        context,
        expectation: TemplateErrorExpectation {
            kind: error_kind,
            contains: error_contains,
        },
    });
}

#[rstest]
fn dirname_filter(filter_workspace: Workspace) {
    with_filter_env(filter_workspace, |root, env| {
        let file = root.join("file");
        let output = render(env, "dirname", "{{ path | dirname }}", &file);
        assert_eq!(output, root.as_str());
    });
}

#[rstest]
fn relative_to_filter(filter_workspace: Workspace) {
    with_filter_env(filter_workspace, |root, env| {
        let dir = Dir::open_ambient_dir(root, ambient_authority()).expect("dir");
        dir.create_dir_all("nested").expect("create nested dir");
        dir.write("nested/file.txt", b"data")
            .expect("write nested file");
        let nested = root.join("nested/file.txt");
        let output = render(
            env,
            "relative_to",
            "{{ path | relative_to(path | dirname) }}",
            &nested,
        );
        assert_eq!(output, "file.txt");
    });
}

#[rstest]
fn relative_to_filter_outside_root(filter_workspace: Workspace) {
    test_filter_error(
        filter_workspace,
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
    );
}

#[rstest]
fn with_suffix_filter(filter_workspace: Workspace) {
    with_filter_env(filter_workspace, |root, env| {
        let file = root.join("file.tar.gz");
        Dir::open_ambient_dir(root, ambient_authority())
            .expect("dir")
            .write("file.tar.gz", b"data")
            .expect("write");
        let first = render(env, "suffix", "{{ path | with_suffix('.log') }}", &file);
        assert_eq!(first, root.join("file.tar.log").as_str());
        let second = render(
            env,
            "suffix_alt",
            "{{ path | with_suffix('.zip', 2) }}",
            &file,
        );
        assert_eq!(second, root.join("file.zip").as_str());
        let third = render(
            env,
            "suffix_count_zero",
            "{{ path | with_suffix('.bak', 0) }}",
            &file,
        );
        assert_eq!(third, root.join("file.tar.gz.bak").as_str());
    });
}

#[rstest]
fn with_suffix_filter_without_separator(filter_workspace: Workspace) {
    with_filter_env(filter_workspace, |root, env| {
        let file = root.join("file");
        let output = render(
            env,
            "suffix_plain",
            "{{ path | with_suffix('.log') }}",
            &file,
        );
        assert_eq!(output, root.join("file.log").as_str());
    });
}

#[rstest]
fn with_suffix_filter_empty_separator(filter_workspace: Workspace) {
    test_filter_error(
        filter_workspace,
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
    );
}

#[rstest]
fn with_suffix_filter_excessive_count(filter_workspace: Workspace) {
    with_filter_env(filter_workspace, |root, env| {
        let file = root.join("file.tar.gz");
        let output = render(
            env,
            "suffix_excessive",
            "{{ path | with_suffix('.bak', 5) }}",
            &file,
        );
        assert_eq!(output, root.join("file.bak").as_str());
    });
}

#[cfg(unix)]
#[rstest]
fn realpath_filter(filter_workspace: Workspace) {
    with_filter_env(filter_workspace, |root, env| {
        let link = root.join("link");
        let output = render(env, "realpath", "{{ path | realpath }}", &link);
        assert_eq!(output, root.join("file").as_str());
    });
}

#[cfg(unix)]
#[rstest]
fn realpath_filter_missing_path(filter_workspace: Workspace) {
    test_filter_error(
        filter_workspace,
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
    );
}

#[cfg(unix)]
#[rstest]
fn realpath_filter_root_path(filter_workspace: Workspace) {
    with_filter_env(filter_workspace, |root, env| {
        let root_path = root
            .ancestors()
            .find(|candidate| candidate.parent().is_none())
            .map(Utf8Path::to_path_buf)
            .expect("root ancestor");
        assert!(
            !root_path.as_str().is_empty(),
            "root path should not be empty",
        );
        let output = render(env, "realpath_root", "{{ path | realpath }}", &root_path);
        assert_eq!(output, root_path.as_str());
    });
}

#[rstest]
fn expanduser_filter(filter_workspace: Workspace) {
    let path = Utf8PathBuf::from("~/workspace");
    assert_filter_success_with_env(
        filter_workspace,
        Some(""),
        "expanduser",
        "{{ path | expanduser }}",
        &path,
        |root| root.join("workspace").as_str().to_owned(),
    );
}

#[rstest]
fn expanduser_filter_non_tilde_path(filter_workspace: Workspace) {
    with_filter_env(filter_workspace, |root, env| {
        let file = root.join("file");
        let output = render(env, "expanduser_plain", "{{ path | expanduser }}", &file);
        assert_eq!(output, file.as_str());
    });
}

#[rstest]
fn expanduser_filter_missing_home(filter_workspace: Workspace) {
    test_filter_error(
        filter_workspace,
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
    );
}

#[rstest]
fn expanduser_filter_user_specific(filter_workspace: Workspace) {
    test_filter_error(
        filter_workspace,
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
    );
}
