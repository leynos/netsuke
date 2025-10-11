use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{ErrorKind, context};
use rstest::rstest;

use super::support::{Workspace, filter_workspace, render, stdlib_env};

#[rstest]
fn contents_and_linecount_filters(filter_workspace: Workspace) {
    let (_temp, root) = filter_workspace;
    let mut env = stdlib_env();
    let file = root.join("file");
    let text = render(&mut env, "contents", "{{ path | contents }}", &file);
    assert_eq!(text, "data");
    let lines = render(
        &mut env,
        "linecount",
        "{{ path | linecount }}",
        &root.join("lines.txt"),
    );
    assert_eq!(lines.parse::<usize>().expect("usize"), 3);

    Dir::open_ambient_dir(&root, ambient_authority())
        .expect("dir")
        .write("empty.txt", b"")
        .expect("empty file");
    let empty_file = root.join("empty.txt");
    let empty_lines = render(
        &mut env,
        "empty_linecount",
        "{{ path | linecount }}",
        &empty_file,
    );
    assert_eq!(empty_lines.parse::<usize>().expect("usize"), 0);
}

#[rstest]
fn contents_filter_unsupported_encoding(filter_workspace: Workspace) {
    let (_temp, root) = filter_workspace;
    let mut env = stdlib_env();
    env.add_template("contents_bad_encoding", "{{ path | contents('latin-1') }}")
        .expect("template");
    let template = env
        .get_template("contents_bad_encoding")
        .expect("get template");
    let file = root.join("file");
    let result = template.render(context!(path => file.as_str()));
    let err = result.expect_err("contents should error on unsupported encoding");
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    assert!(
        err.to_string().contains("unsupported encoding"),
        "error should mention unsupported encoding",
    );
}

#[rstest]
fn size_filter(filter_workspace: Workspace) {
    let (_temp, root) = filter_workspace;
    let mut env = stdlib_env();
    let file = root.join("file");
    let size = render(&mut env, "size", "{{ path | size }}", &file);
    assert_eq!(size.parse::<u64>().expect("u64"), 4);
}

#[rstest]
fn size_filter_missing_file(filter_workspace: Workspace) {
    let (_temp, root) = filter_workspace;
    let mut env = stdlib_env();
    env.add_template("size_missing", "{{ path | size }}")
        .expect("template");
    let template = env.get_template("size_missing").expect("get template");
    let missing = root.join("does_not_exist");
    let result = template.render(context!(path => missing.as_str()));
    let err = result.expect_err("size should error for missing file");
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    assert!(
        err.to_string().contains("does_not_exist") || err.to_string().contains("not found"),
        "error should mention missing file: {err}",
    );
}
