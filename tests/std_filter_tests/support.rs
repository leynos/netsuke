use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{Environment, context};
use netsuke::stdlib::{self, StdlibConfig, StdlibState};
use rstest::fixture;
use tempfile::tempdir;

pub(crate) use test_support::{EnvVarGuard, env_lock::EnvLock};

pub(crate) type Workspace = (tempfile::TempDir, Utf8PathBuf);

pub(crate) fn register_template(
    env: &mut Environment<'_>,
    name: impl Into<String>,
    source: impl Into<String>,
) {
    let name = name.into();
    let source = source.into();
    env.add_template_owned(name, source).expect("template");
}

pub(crate) fn stdlib_env_with_config(config: StdlibConfig) -> (Environment<'static>, StdlibState) {
    let mut env = Environment::new();
    let state = stdlib::register_with_config(&mut env, config);
    (env, state)
}

pub(crate) fn stdlib_env_with_state() -> (Environment<'static>, StdlibState) {
    stdlib_env_with_config(StdlibConfig::default())
}

pub(crate) fn stdlib_env() -> Environment<'static> {
    let (env, _) = stdlib_env_with_state();
    env
}

#[fixture]
pub(crate) fn filter_workspace() -> Workspace {
    let temp = tempdir().expect("tempdir");
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");
    let dir = Dir::open_ambient_dir(&root, ambient_authority()).expect("dir");
    dir.write("file", b"data").expect("file");
    #[cfg(unix)]
    dir.symlink("file", "link").expect("symlink");
    #[cfg(not(unix))]
    dir.write("link", b"data").expect("link copy");
    dir.write("lines.txt", b"one\ntwo\nthree\n").expect("lines");
    (temp, root)
}

pub(crate) fn render<'a>(
    env: &mut Environment<'a>,
    name: &'a str,
    template: &'a str,
    path: &Utf8PathBuf,
) -> String {
    env.add_template(name, template).expect("template");
    env.get_template(name)
        .expect("get template")
        .render(context!(path => path.as_str()))
        .expect("render")
}
