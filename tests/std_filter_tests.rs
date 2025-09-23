use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{Environment, context};
use netsuke::stdlib;
use rstest::{fixture, rstest};
use tempfile::tempdir;
use test_support::{EnvVarGuard, env_lock::EnvLock};

#[fixture]
fn filter_workspace() -> (tempfile::TempDir, Utf8PathBuf) {
    let temp = tempdir().expect("tempdir");
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");
    let dir = Dir::open_ambient_dir(&root, ambient_authority()).expect("dir");
    dir.write("file", b"data").expect("file");
    dir.symlink("file", "link").expect("symlink");
    dir.write("lines.txt", b"one\ntwo\nthree\n").expect("lines");
    (temp, root)
}

fn render<'a>(
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

#[rstest]
fn basename_filter(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
    let (_temp, root) = filter_workspace;
    let mut env = Environment::new();
    stdlib::register(&mut env);
    let file = root.join("file");
    let output = render(&mut env, "basename", "{{ path | basename }}", &file);
    assert_eq!(output, "file");
}

#[rstest]
fn dirname_filter(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
    let (_temp, root) = filter_workspace;
    let mut env = Environment::new();
    stdlib::register(&mut env);
    let file = root.join("file");
    let output = render(&mut env, "dirname", "{{ path | dirname }}", &file);
    assert_eq!(output, root.as_str());
}

#[rstest]
fn with_suffix_filter(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
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
}

#[rstest]
fn realpath_filter(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
    let (_temp, root) = filter_workspace;
    let mut env = Environment::new();
    stdlib::register(&mut env);
    let link = root.join("link");
    let output = render(&mut env, "realpath", "{{ path | realpath }}", &link);
    assert_eq!(output, root.join("file").as_str());
}

#[rstest]
fn contents_and_linecount_filters(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
    let (_temp, root) = filter_workspace;
    let mut env = Environment::new();
    stdlib::register(&mut env);
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
}

#[rstest]
fn hash_and_digest_filters(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
    let (_temp, root) = filter_workspace;
    let mut env = Environment::new();
    stdlib::register(&mut env);
    let file = root.join("file");
    let hash = render(&mut env, "hash", "{{ path | hash }}", &file);
    assert_eq!(
        hash,
        "3a6eb0790f39ac87c94f3856b2dd2c5d110e6811602261a9a923d3bb23adc8b7"
    );
    let digest = render(&mut env, "digest", "{{ path | digest(8) }}", &file);
    assert_eq!(digest, "3a6eb079");
}

#[rstest]
fn size_filter(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
    let (_temp, root) = filter_workspace;
    let mut env = Environment::new();
    stdlib::register(&mut env);
    let file = root.join("file");
    let size = render(&mut env, "size", "{{ path | size }}", &file);
    assert_eq!(size.parse::<u64>().expect("u64"), 4);
}

#[rstest]
fn expanduser_filter(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
    let (_temp, root) = filter_workspace;
    let mut env = Environment::new();
    stdlib::register(&mut env);
    let _lock = EnvLock::acquire();
    let _guard = EnvVarGuard::set("HOME", root.as_str());
    let home = render(
        &mut env,
        "expanduser",
        "{{ path | expanduser }}",
        &Utf8PathBuf::from("~/workspace"),
    );
    assert_eq!(home, root.join("workspace").as_str());
}
