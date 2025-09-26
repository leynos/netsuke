use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{Environment, ErrorKind, context};
use netsuke::stdlib;
use rstest::{fixture, rstest};
use std::cell::RefCell;
use tempfile::tempdir;
use test_support::{EnvVarGuard, env_lock::EnvLock};

thread_local! {
    static TEMPLATE_STORAGE: RefCell<Vec<(Box<str>, Box<str>)>> = const { RefCell::new(Vec::new()) };
}

fn register_template(
    env: &mut Environment<'_>,
    name: impl Into<String>,
    source: impl Into<String>,
) {
    TEMPLATE_STORAGE.with(|storage| {
        let (name_ptr, source_ptr) = {
            let mut storage = storage.borrow_mut();
            storage.push((name.into().into_boxed_str(), source.into().into_boxed_str()));
            let (name, source) = storage.last().expect("template storage entry");
            (
                std::ptr::from_ref(name.as_ref()),
                std::ptr::from_ref(source.as_ref()),
            )
        };
        // SAFETY: the pointers originate from boxed strings stored in the
        // thread-local registry. They remain valid for the duration of the
        // process, so treating them as `'static` references is sound.
        unsafe {
            env.add_template(&*name_ptr, &*source_ptr)
                .expect("template");
        }
    });
}

#[fixture]
fn filter_workspace() -> (tempfile::TempDir, Utf8PathBuf) {
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
fn dirname_filter(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
    let (_temp, root) = filter_workspace;
    let mut env = Environment::new();
    stdlib::register(&mut env);
    let file = root.join("file");
    let output = render(&mut env, "dirname", "{{ path | dirname }}", &file);
    assert_eq!(output, root.as_str());
}

#[rstest]
fn relative_to_filter(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
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
fn relative_to_filter_outside_root(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
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
    let third = render(
        &mut env,
        "suffix_count_zero",
        "{{ path | with_suffix('.bak', 0) }}",
        &file,
    );
    assert_eq!(third, root.join("file.tar.gz.bak").as_str());
}

#[rstest]
fn with_suffix_filter_without_separator(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
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
fn with_suffix_filter_empty_separator(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
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
fn with_suffix_filter_excessive_count(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
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
fn realpath_filter(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
    let (_temp, root) = filter_workspace;
    let mut env = Environment::new();
    stdlib::register(&mut env);
    let link = root.join("link");
    let output = render(&mut env, "realpath", "{{ path | realpath }}", &link);
    assert_eq!(output, root.join("file").as_str());
}

#[cfg(unix)]
#[rstest]
fn realpath_filter_missing_path(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
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
fn realpath_filter_root_path(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
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
fn contents_filter_unsupported_encoding(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
    let (_temp, root) = filter_workspace;
    let mut env = Environment::new();
    stdlib::register(&mut env);
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
#[case(
    "sha256",
    "3a6eb0790f39ac87c94f3856b2dd2c5d110e6811602261a9a923d3bb23adc8b7",
    "3a6eb079"
)]
#[case(
    "sha512",
    "77c7ce9a5d86bb386d443bb96390faa120633158699c8844c30b13ab0bf92760b7e4416aea397db91b4ac0e5dd56b8ef7e4b066162ab1fdc088319ce6defc876",
    "77c7ce9a"
)]
#[case(
    "sha256-empty",
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "e3b0c442"
)]
#[case(
    "sha512-empty",
    "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e",
    "cf83e135"
)]
#[cfg_attr(
    feature = "legacy-digests",
    case("sha1", "a17c9aaa61e80a1bf71d0d850af4e5baa9800bbd", "a17c9aaa",)
)]
#[cfg_attr(
    feature = "legacy-digests",
    case("sha1-empty", "da39a3ee5e6b4b0d3255bfef95601890afd80709", "da39a3ee",)
)]
#[cfg_attr(
    feature = "legacy-digests",
    case("md5", "8d777f385d3dfec8815d20f7496026dc", "8d777f38",)
)]
#[cfg_attr(
    feature = "legacy-digests",
    case("md5-empty", "d41d8cd98f00b204e9800998ecf8427e", "d41d8cd9",)
)]
fn hash_and_digest_filters(
    filter_workspace: (tempfile::TempDir, Utf8PathBuf),
    #[case] alg: &str,
    #[case] expected_hash: &str,
    #[case] expected_digest: &str,
) {
    let (_temp, root) = filter_workspace;
    let mut env = Environment::new();
    stdlib::register(&mut env);
    let dir = Dir::open_ambient_dir(&root, ambient_authority()).expect("dir");

    let (file, algorithm) = alg.strip_suffix("-empty").map_or_else(
        || (root.join("file"), alg),
        |stripped| {
            let relative = format!("{stripped}_empty");
            dir.write(relative.as_str(), b"")
                .expect("create empty file");
            (root.join(relative.as_str()), stripped)
        },
    );

    let hash_template_name = format!("hash_{alg}");
    let hash_template = format!("{{{{ path | hash('{algorithm}') }}}}");
    register_template(&mut env, hash_template_name.as_str(), hash_template);
    let hash_result = env
        .get_template(hash_template_name.as_str())
        .expect("get template")
        .render(context!(path => file.as_str()))
        .expect("render hash");
    assert_eq!(hash_result, expected_hash);

    let digest_template_name = format!("digest_{alg}");
    let digest_template = format!("{{{{ path | digest(8, '{algorithm}') }}}}");
    register_template(&mut env, digest_template_name.as_str(), digest_template);
    let digest_result = env
        .get_template(digest_template_name.as_str())
        .expect("get template")
        .render(context!(path => file.as_str()))
        .expect("render digest");
    assert_eq!(digest_result, expected_digest);
}

#[cfg(not(feature = "legacy-digests"))]
#[rstest]
fn hash_filter_legacy_algorithms_disabled(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
    let (_temp, root) = filter_workspace;
    let mut env = Environment::new();
    stdlib::register(&mut env);

    register_template(&mut env, "hash_sha1", "{{ path | hash('sha1') }}");
    let template = env.get_template("hash_sha1").expect("get template");
    let result = template.render(context!(path => root.join("file").as_str()));
    let err = result.expect_err("hash should require the legacy-digests feature for sha1");
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    assert!(
        err.to_string().contains("enable feature 'legacy-digests'"),
        "error should mention legacy feature: {err}",
    );
}

#[rstest]
fn hash_filter_rejects_unknown_algorithm(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
    let (_temp, root) = filter_workspace;
    let mut env = Environment::new();
    stdlib::register(&mut env);
    let file = root.join("file");

    register_template(&mut env, "hash_unknown", "{{ path | hash('whirlpool') }}");
    let hash_template = env.get_template("hash_unknown").expect("get template");
    let hash_result = hash_template.render(context!(path => file.as_str()));
    let hash_err = hash_result.expect_err("hash should reject unsupported algorithms");
    assert_eq!(hash_err.kind(), ErrorKind::InvalidOperation);
    assert!(
        hash_err
            .to_string()
            .contains("unsupported hash algorithm 'whirlpool'"),
        "error should mention unsupported algorithm: {hash_err}",
    );

    register_template(
        &mut env,
        "digest_unknown",
        "{{ path | digest(8, 'whirlpool') }}",
    );
    let digest_template = env.get_template("digest_unknown").expect("get template");
    let digest_result = digest_template.render(context!(path => file.as_str()));
    let digest_err = digest_result.expect_err("digest should reject unsupported algorithms");
    assert_eq!(digest_err.kind(), ErrorKind::InvalidOperation);
    assert!(
        digest_err
            .to_string()
            .contains("unsupported hash algorithm 'whirlpool'"),
        "error should mention unsupported algorithm: {digest_err}",
    );
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
fn size_filter_missing_file(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
    let (_temp, root) = filter_workspace;
    let mut env = Environment::new();
    stdlib::register(&mut env);
    register_template(&mut env, "size_missing", "{{ path | size }}");
    let missing = root.join("does_not_exist");
    let template = env.get_template("size_missing").expect("get template");
    let result = template.render(context!(path => missing.as_str()));
    let err = result.expect_err("size should error for missing file");
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    assert!(
        err.to_string().contains("does_not_exist") || err.to_string().contains("not found"),
        "error should mention missing file: {err}",
    );
}

#[rstest]
fn expanduser_filter(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
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
fn expanduser_filter_non_tilde_path(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
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
fn expanduser_filter_missing_home(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
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
fn expanduser_filter_user_specific(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
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
