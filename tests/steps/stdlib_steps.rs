//! Behavioural steps exercising the template stdlib path and file filters.
//! Prepare a temporary workspace, render templates with stdlib registration,
//! and assert expected outputs or `MiniJinja` errors.
use crate::CliWorld;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use cucumber::{given, then, when};
use minijinja::{Environment, context};
use netsuke::stdlib;
use std::ffi::OsStr;
use test_support::env::set_var;

const LINES_FIXTURE: &str = concat!(
    "one
", "two
", "three
"
);

fn ensure_workspace(world: &mut CliWorld) -> Utf8PathBuf {
    if let Some(root) = &world.stdlib_root {
        return root.clone();
    }
    let temp = tempfile::tempdir().expect("create stdlib workspace");
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");
    let handle = Dir::open_ambient_dir(&root, ambient_authority()).expect("open workspace");
    handle.write("file", b"data").expect("write file");
    handle
        .write("lines.txt", LINES_FIXTURE.as_bytes())
        .expect("write lines fixture");
    #[cfg(unix)]
    handle.symlink("file", "link").expect("create symlink");
    #[cfg(not(unix))]
    handle.write("link", b"data").expect("write link fixture");
    world.temp = Some(temp);
    world.stdlib_root = Some(root.clone());
    root
}

fn render_template(world: &mut CliWorld, template: &str, path: &Utf8Path) {
    let mut env = Environment::new();
    stdlib::register(&mut env);
    let render = env.render_str(template, context!(path => path.as_str()));
    match render {
        Ok(output) => {
            world.stdlib_output = Some(output);
            world.stdlib_error = None;
        }
        Err(err) => {
            world.stdlib_output = None;
            world.stdlib_error = Some(err.to_string());
        }
    }
}

#[given("a stdlib workspace")]
fn stdlib_workspace(world: &mut CliWorld) {
    let root = ensure_workspace(world);
    world.stdlib_root = Some(root);
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned capture arguments"
)]
#[given(regex = r#"^the stdlib file "(.+)" contains "(.+)"$"#)]
fn write_stdlib_file(world: &mut CliWorld, path: Utf8PathBuf, contents: String) {
    let root = ensure_workspace(world);
    let handle = Dir::open_ambient_dir(&root, ambient_authority()).expect("open workspace");
    let relative = path.as_path();
    if let Some(parent) = relative.parent().filter(|p| !p.as_str().is_empty()) {
        handle
            .create_dir_all(parent)
            .expect("create fixture directories");
    }
    handle
        .write(relative, contents.as_bytes())
        .expect("write stdlib file");
}

#[given("HOME points to the stdlib workspace root")]
fn home_points_to_stdlib_root(world: &mut CliWorld) {
    let root = ensure_workspace(world);
    let os_root = OsStr::new(root.as_str());
    let previous = set_var("HOME", os_root);
    world.env_vars.entry("HOME".into()).or_insert(previous);
    #[cfg(windows)]
    {
        let previous = set_var("USERPROFILE", os_root);
        world
            .env_vars
            .entry("USERPROFILE".into())
            .or_insert(previous);
    }
    world.stdlib_root = Some(root);
}

#[when(regex = r#"^I render "(.+)" with stdlib path "(.+)"$"#)]
fn render_stdlib_template(world: &mut CliWorld, template: String, path: Utf8PathBuf) {
    let root = ensure_workspace(world);
    let path_str = path.as_str();
    let is_home_expansion = path_str.starts_with('~');
    let is_absolute = path.is_absolute();
    let target = if is_home_expansion || is_absolute {
        path
    } else {
        root.join(path_str)
    };
    render_template(world, template.as_str(), target.as_path());
    drop(template);
}

#[then(regex = r#"^the stdlib output is "(.+)"$"#)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned capture arguments"
)]
fn assert_stdlib_output(world: &mut CliWorld, expected: String) {
    let output = world
        .stdlib_output
        .as_ref()
        .expect("expected stdlib output");
    assert_eq!(output, &expected);
}

fn stdlib_root_and_output(world: &CliWorld) -> (&Utf8Path, &str) {
    let root = world
        .stdlib_root
        .as_ref()
        .expect("expected stdlib workspace root")
        .as_path();
    let output = world
        .stdlib_output
        .as_deref()
        .expect("expected stdlib output");
    (root, output)
}

#[then(regex = r#"^the stdlib error contains "(.+)"$"#)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned capture arguments"
)]
fn assert_stdlib_error(world: &mut CliWorld, fragment: String) {
    let error = world.stdlib_error.as_ref().expect("expected stdlib error");
    assert!(
        error.contains(&fragment),
        "error `{error}` should contain `{fragment}`",
    );
}

#[then("the stdlib output equals the workspace root")]
fn assert_stdlib_output_is_root(world: &mut CliWorld) {
    let (root, output) = stdlib_root_and_output(world);
    assert_eq!(output, root.as_str());
}

#[then(regex = r#"^the stdlib output is the workspace path "(.+)"$"#)]
fn assert_stdlib_output_is_workspace_path(world: &mut CliWorld, relative: Utf8PathBuf) {
    let (root, output) = stdlib_root_and_output(world);
    let expected = root.join(relative);
    assert_eq!(output, expected.as_str());
}
