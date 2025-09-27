use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{Environment, ErrorKind, context};
use netsuke::stdlib;
use rstest::rstest;

use super::support::{
    EnvLock, EnvVarGuard, Workspace, filter_workspace, register_template, render,
};

#[rstest]
fn dirname_filter(filter_workspace: Workspace) {
    let (_temp, root) = filter_workspace;
    let mut env = Environment::new();
    stdlib::register(&mut env);
    let file = root.join("file");
    let output = render(&mut env, "dirname", "{{ path | dirname }}", &file);
    assert_eq!(output, root.as_str());
}

#[rstest]
fn relative_to_filter(filter_workspace: Workspace) {
    let (_temp, root) = filter_workspace;
    let mut env = Environment::new();
    stdlib::register(&mut env);
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
    let mut env = Environment::new();
    stdlib::register(&mut env);
    register_template(
        &mut env,
        "relative_to_fail",
        "{{ path | relative_to(root) }}",
    );
    let template = env.get_template("relative_to_fail").expect("get template");
    let file = root.join("file");
    let other_root = root.join("other");
    let result = template.render(context!(path => file.as_str(), root => other_root.as_str()));
    let err = result.expect_err("relative_to should reject unrelated paths");
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    assert!(
        err.to_string().contains("is not relative"),
        "error should mention missing relationship: {err}"
    );
}

#[rstest]
fn with_suffix_filter(filter_workspace: Workspace) {
    let (_temp, root) = filter_workspace;
    let mut env = Environment::new();
    stdlib::register(&mut env);
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
    let mut env = Environment::new();
    stdlib::register(&mut env);
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
    let mut env = Environment::new();
    stdlib::register(&mut env);
    env.add_template(
        "suffix_empty_sep",
        "{{ path | with_suffix('.log', 1, '') }}",
    )
    .expect("template");
    let template = env.get_template("suffix_empty_sep").expect("get template");
    let file = root.join("file.tar.gz");
    let result = template.render(context!(path => file.as_str()));
    let err = result.expect_err("with_suffix should reject empty separator");
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    assert!(
        err.to_string().contains("non-empty separator"),
        "error should mention separator requirement",
    );
}

#[rstest]
fn with_suffix_filter_excessive_count(filter_workspace: Workspace) {
    let (_temp, root) = filter_workspace;
    let mut env = Environment::new();
    stdlib::register(&mut env);
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
    let mut env = Environment::new();
    stdlib::register(&mut env);
    let link = root.join("link");
    let output = render(&mut env, "realpath", "{{ path | realpath }}", &link);
    assert_eq!(output, root.join("file").as_str());
}

#[cfg(unix)]
#[rstest]
fn realpath_filter_missing_path(filter_workspace: Workspace) {
    let (_temp, root) = filter_workspace;
    let mut env = Environment::new();
    stdlib::register(&mut env);
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
    let mut env = Environment::new();
    stdlib::register(&mut env);
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
    let mut env = Environment::new();
    stdlib::register(&mut env);
    let _lock = EnvLock::acquire();
    let _home_guard = EnvVarGuard::set("HOME", root.as_str());
    let _profile_guard = EnvVarGuard::remove("USERPROFILE");
    let _drive_guard = EnvVarGuard::remove("HOMEDRIVE");
    let _path_guard = EnvVarGuard::remove("HOMEPATH");
    let _share_guard = EnvVarGuard::remove("HOMESHARE");
    let home = render(
        &mut env,
        "expanduser",
        "{{ path | expanduser }}",
        &Utf8PathBuf::from("~/workspace"),
    );
    assert_eq!(home, root.join("workspace").as_str());
}

#[rstest]
fn expanduser_filter_non_tilde_path(filter_workspace: Workspace) {
    let (_temp, root) = filter_workspace;
    let mut env = Environment::new();
    stdlib::register(&mut env);
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
    let mut env = Environment::new();
    stdlib::register(&mut env);
    let _lock = EnvLock::acquire();
    let _home_guard = EnvVarGuard::remove("HOME");
    let _profile_guard = EnvVarGuard::remove("USERPROFILE");
    let _drive_guard = EnvVarGuard::remove("HOMEDRIVE");
    let _path_guard = EnvVarGuard::remove("HOMEPATH");
    let _share_guard = EnvVarGuard::remove("HOMESHARE");
    env.add_template("expanduser_missing_home", "{{ path | expanduser }}")
        .expect("template");
    let template = env
        .get_template("expanduser_missing_home")
        .expect("get template");
    let result = template.render(context!(path => "~/workspace"));
    let err = result.expect_err("expanduser should error when HOME is unset");
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    assert!(
        err.to_string()
            .contains("no home directory environment variables are set"),
        "error should mention missing HOME",
    );
}

#[rstest]
fn expanduser_filter_user_specific(filter_workspace: Workspace) {
    let (_temp, root) = filter_workspace;
    let mut env = Environment::new();
    stdlib::register(&mut env);
    let _lock = EnvLock::acquire();
    let _home_guard = EnvVarGuard::set("HOME", root.as_str());
    let _profile_guard = EnvVarGuard::remove("USERPROFILE");
    let _drive_guard = EnvVarGuard::remove("HOMEDRIVE");
    let _path_guard = EnvVarGuard::remove("HOMEPATH");
    let _share_guard = EnvVarGuard::remove("HOMESHARE");
    env.add_template("expanduser_user_specific", "{{ path | expanduser }}")
        .expect("template");
    let template = env
        .get_template("expanduser_user_specific")
        .expect("get template");
    let result = template.render(context!(path => "~otheruser/workspace"));
    let err = result.expect_err("expanduser should reject ~user expansion");
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    assert!(
        err.to_string()
            .contains("user-specific ~ expansion is unsupported"),
        "error should mention unsupported user expansion",
    );
}
