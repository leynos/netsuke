use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{Environment, ErrorKind, context};
use rstest::rstest;
use serde_json::json;

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

/// Helper for error testing with custom template
fn assert_template_error(
    env: &mut Environment<'_>,
    template_name: &str,
    template_content: &str,
    context_data: serde_json::Value,
    expected_kind: ErrorKind,
    error_contains: &str,
) {
    register_template(env, template_name, template_content);
    let template = env.get_template(template_name).expect("get template");
    let result = template.render(context_data);
    let err = result.expect_err("template rendering should fail");
    assert_eq!(err.kind(), expected_kind);
    assert!(
        err.to_string().contains(error_contains),
        "error should mention {error_contains}: {err}"
    );
}

#[rstest]
fn dirname_filter(filter_workspace: Workspace) {
    let (_temp, root) = filter_workspace;
    let mut env = setup_filter_env();
    let file = root.join("file");
    let output = render(&mut env, "dirname", "{{ path | dirname }}", &file);
    assert_eq!(output, root.as_str());
}

#[rstest]
fn relative_to_filter(filter_workspace: Workspace) {
    let (_temp, root) = filter_workspace;
    let mut env = setup_filter_env();
    let dir = Dir::open_ambient_dir(&root, ambient_authority()).expect("dir");
    dir.create_dir_all("nested").expect("create nested dir");
    dir.write("nested/file.txt", b"data")
        .expect("write nested file");
    let nested = root.join("nested/file.txt");
    let output = render(
        &mut env,
        "relative_to",
        "{{ path | relative_to(path | dirname) }}",
        &nested,
    );
    assert_eq!(output, "file.txt");
}

#[rstest]
fn relative_to_filter_outside_root(filter_workspace: Workspace) {
    let (_temp, root) = filter_workspace;
    let mut env = setup_filter_env();
    let file = root.join("file");
    let other_root = root.join("other");
    assert_template_error(
        &mut env,
        "relative_to_fail",
        "{{ path | relative_to(root) }}",
        json!({
            "path": file.as_str(),
            "root": other_root.as_str(),
        }),
        ErrorKind::InvalidOperation,
        "is not relative",
    );
}

#[rstest]
fn with_suffix_filter(filter_workspace: Workspace) {
    let (_temp, root) = filter_workspace;
    let mut env = stdlib_env();
    let file = root.join("file.tar.gz");
    Dir::open_ambient_dir(&root, ambient_authority())
        .expect("dir")
        .write("file.tar.gz", b"data")
        .expect("write");
    let first = render(
        &mut env,
        "suffix",
        "{{ path | with_suffix('.log') }}",
        &file,
    );
    assert_eq!(first, root.join("file.tar.log").as_str());
    let second = render(
        &mut env,
        "suffix_alt",
        "{{ path | with_suffix('.zip', 2) }}",
        &file,
    );
    assert_eq!(second, root.join("file.zip").as_str());
    let third = render(
        &mut env,
        "suffix_count_zero",
        "{{ path | with_suffix('.bak', 0) }}",
        &file,
    );
    assert_eq!(third, root.join("file.tar.gz.bak").as_str());
}

#[rstest]
fn with_suffix_filter_without_separator(filter_workspace: Workspace) {
    let (_temp, root) = filter_workspace;
    let mut env = stdlib_env();
    let file = root.join("file");
    let output = render(
        &mut env,
        "suffix_plain",
        "{{ path | with_suffix('.log') }}",
        &file,
    );
    assert_eq!(output, root.join("file.log").as_str());
}

#[rstest]
fn with_suffix_filter_empty_separator(filter_workspace: Workspace) {
    let (_temp, root) = filter_workspace;
    let mut env = setup_filter_env();
    let file = root.join("file.tar.gz");
    assert_template_error(
        &mut env,
        "suffix_empty_sep",
        "{{ path | with_suffix('.log', 1, '') }}",
        json!({
            "path": file.as_str(),
        }),
        ErrorKind::InvalidOperation,
        "non-empty separator",
    );
}

#[rstest]
fn with_suffix_filter_excessive_count(filter_workspace: Workspace) {
    let (_temp, root) = filter_workspace;
    let mut env = stdlib_env();
    let file = root.join("file.tar.gz");
    let output = render(
        &mut env,
        "suffix_excessive",
        "{{ path | with_suffix('.bak', 5) }}",
        &file,
    );
    assert_eq!(output, root.join("file.bak").as_str());
}

#[cfg(unix)]
#[rstest]
fn realpath_filter(filter_workspace: Workspace) {
    let (_temp, root) = filter_workspace;
    let mut env = stdlib_env();
    let link = root.join("link");
    let output = render(&mut env, "realpath", "{{ path | realpath }}", &link);
    assert_eq!(output, root.join("file").as_str());
}

#[cfg(unix)]
#[rstest]
fn realpath_filter_missing_path(filter_workspace: Workspace) {
    let (_temp, root) = filter_workspace;
    let mut env = stdlib_env();
    env.add_template("realpath_missing", "{{ path | realpath }}")
        .expect("template");
    let template = env.get_template("realpath_missing").expect("get template");
    let missing = root.join("missing");
    let result = template.render(context!(path => missing.as_str()));
    let err = result.expect_err("realpath should error for missing path");
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    assert!(
        err.to_string().contains("not found"),
        "error should mention missing path",
    );
}

#[cfg(unix)]
#[rstest]
fn realpath_filter_root_path(filter_workspace: Workspace) {
    let (_temp, root) = filter_workspace;
    let mut env = stdlib_env();
    let root_path = root
        .ancestors()
        .find(|candidate| candidate.parent().is_none())
        .map(Utf8Path::to_path_buf)
        .expect("root ancestor");
    assert!(
        !root_path.as_str().is_empty(),
        "root path should not be empty",
    );
    let output = render(
        &mut env,
        "realpath_root",
        "{{ path | realpath }}",
        &root_path,
    );
    assert_eq!(output, root_path.as_str());
}

#[rstest]
fn expanduser_filter(filter_workspace: Workspace) {
    let (_temp, root) = filter_workspace;
    with_clean_env_vars(Some(root.as_str()), || {
        let mut env = setup_filter_env();
        let home = render(
            &mut env,
            "expanduser",
            "{{ path | expanduser }}",
            &Utf8PathBuf::from("~/workspace"),
        );
        assert_eq!(home, root.join("workspace").as_str());
    });
}

#[rstest]
fn expanduser_filter_non_tilde_path(filter_workspace: Workspace) {
    let (_temp, root) = filter_workspace;
    let mut env = setup_filter_env();
    let file = root.join("file");
    let output = render(
        &mut env,
        "expanduser_plain",
        "{{ path | expanduser }}",
        &file,
    );
    assert_eq!(output, file.as_str());
}

#[rstest]
fn expanduser_filter_missing_home(filter_workspace: Workspace) {
    let (_temp, _root) = filter_workspace;
    with_clean_env_vars(None, || {
        let mut env = setup_filter_env();
        assert_template_error(
            &mut env,
            "expanduser_missing_home",
            "{{ path | expanduser }}",
            json!({
                "path": "~/workspace",
            }),
            ErrorKind::InvalidOperation,
            "no home directory environment variables are set",
        );
    });
}

#[rstest]
fn expanduser_filter_user_specific(filter_workspace: Workspace) {
    let (_temp, root) = filter_workspace;
    with_clean_env_vars(Some(root.as_str()), || {
        let mut env = setup_filter_env();
        assert_template_error(
            &mut env,
            "expanduser_user_specific",
            "{{ path | expanduser }}",
            json!({
                "path": "~otheruser/workspace",
            }),
            ErrorKind::InvalidOperation,
            "user-specific ~ expansion is unsupported",
        );
    });
}
