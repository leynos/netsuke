use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{Environment, ErrorKind, context};
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

fn register_template(
    env: &mut Environment<'_>,
    name: impl Into<String>,
    source: impl Into<String>,
) {
    let leaked_name = Box::leak(name.into().into_boxed_str());
    let leaked_source = Box::leak(source.into().into_boxed_str());
    env.add_template(leaked_name, leaked_source)
        .expect("template");
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
#[cfg_attr(
    feature = "legacy-digests",
    case("sha1", "a17c9aaa61e80a1bf71d0d850af4e5baa9800bbd", "a17c9aaa",)
)]
#[cfg_attr(
    feature = "legacy-digests",
    case("md5", "8d777f385d3dfec8815d20f7496026dc", "8d777f38",)
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
    let file = root.join("file");

    let hash_template_name = format!("hash_{alg}");
    let hash_template = format!("{{{{ path | hash('{alg}') }}}}");
    register_template(&mut env, hash_template_name.as_str(), hash_template);
    let hash_result = env
        .get_template(hash_template_name.as_str())
        .expect("get template")
        .render(context!(path => file.as_str()))
        .expect("render hash");
    assert_eq!(hash_result, expected_hash);

    let digest_template_name = format!("digest_{alg}");
    let digest_template = format!("{{{{ path | digest(8, '{alg}') }}}}");
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

#[rstest]
fn expanduser_filter_missing_home(filter_workspace: (tempfile::TempDir, Utf8PathBuf)) {
    let (_temp, _root) = filter_workspace;
    let mut env = Environment::new();
    stdlib::register(&mut env);
    let _lock = EnvLock::acquire();
    let _home_guard = EnvVarGuard::remove("HOME");
    let _profile_guard = EnvVarGuard::remove("USERPROFILE");
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
            .contains("neither HOME nor USERPROFILE is set"),
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
