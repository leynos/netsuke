use crate::CliWorld;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use cucumber::{given, then, when};
use minijinja::{Environment, context};
use netsuke::stdlib;

fn ensure_workspace(world: &mut CliWorld) -> Utf8PathBuf {
    if let Some(root) = &world.stdlib_root {
        return root.clone();
    }
    let temp = tempfile::tempdir().expect("create stdlib workspace");
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");
    let handle = Dir::open_ambient_dir(&root, ambient_authority()).expect("open workspace");
    handle.write("file", b"data").expect("write file");
    world.temp = Some(temp);
    world.stdlib_root = Some(root.clone());
    root
}

fn render_template(world: &mut CliWorld, template: &str, path: &Utf8Path) {
    let mut env = Environment::new();
    stdlib::register(&mut env);
    env.add_template("scenario", template)
        .expect("add template");
    let render = env
        .get_template("scenario")
        .expect("get template")
        .render(context!(path => path.as_str()));
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
    // record root to reuse in subsequent steps
    world.stdlib_root = Some(root);
}

#[when(regex = r#"^I render "(.+)" with stdlib path "(.+)"$"#)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
fn render_stdlib_template(world: &mut CliWorld, template: String, path: String) {
    let root = ensure_workspace(world);
    let target = root.join(path);
    render_template(world, &template, &target);
}

#[then(regex = r#"^the stdlib output is "(.+)"$"#)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
fn assert_stdlib_output(world: &mut CliWorld, expected: String) {
    let output = world
        .stdlib_output
        .as_ref()
        .expect("expected stdlib output");
    assert_eq!(output, &expected);
}

#[then(regex = r#"^the stdlib error contains "(.+)"$"#)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
fn assert_stdlib_error(world: &mut CliWorld, fragment: String) {
    let error = world.stdlib_error.as_ref().expect("expected stdlib error");
    assert!(
        error.contains(&fragment),
        "error `{error}` should contain `{fragment}`",
    );
}
