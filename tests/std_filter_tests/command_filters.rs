use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{ErrorKind, context};
use rstest::rstest;
use tempfile::tempdir;
use test_support::command_helper::compile_uppercase_helper;

use super::support::stdlib_env_with_state;

#[cfg(windows)]
use {
    super::support::{EnvLock, EnvVarGuard},
    rstest::fixture,
    std::{ffi::OsString, process::Command},
};

#[rstest]
fn shell_filter_marks_templates_impure() {
    let temp = tempdir().expect("tempdir");
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");
    let dir = Dir::open_ambient_dir(&root, ambient_authority()).expect("dir");
    let helper = compile_uppercase_helper(&dir, &root, "cmd_upper");
    let command = format!("\"{}\"", helper.as_str());

    let (mut env, state) = stdlib_env_with_state();
    state.reset_impure();
    env.add_template("shell", "{{ 'hello' | shell(cmd) | trim }}")
        .expect("template");
    let template = env.get_template("shell").expect("get template");
    let rendered = template.render(context!(cmd => command)).expect("render");
    assert_eq!(rendered, "HELLO");
    assert!(
        state.is_impure(),
        "shell filter should mark template impure"
    );
}

#[rstest]
fn shell_filter_surfaces_command_failures() {
    let (mut env, state) = stdlib_env_with_state();
    state.reset_impure();
    env.add_template("shell_fail", "{{ 'data' | shell('false') }}")
        .expect("template");
    let template = env.get_template("shell_fail").expect("get template");
    let result = template.render(context! {});
    let err = result.expect_err("shell should propagate failures");
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    assert!(
        state.is_impure(),
        "failure should still mark template impure"
    );
    assert!(
        err.to_string().contains("shell command")
            || err.to_string().contains("failed")
            || err.to_string().contains("error"),
        "error should indicate command failure: {err}",
    );
}

#[cfg(unix)]
#[rstest]
fn shell_filter_times_out_long_commands() {
    let (mut env, state) = stdlib_env_with_state();
    state.reset_impure();
    env.add_template("shell_timeout", "{{ '' | shell('sleep 10') }}")
        .expect("template");
    let template = env.get_template("shell_timeout").expect("get template");
    let err = template
        .render(context! {})
        .expect_err("sleep should exceed shell timeout");
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    assert!(state.is_impure(), "timeout should mark template impure");
    assert!(
        err.to_string().contains("timed out"),
        "timeout error should mention duration: {err}",
    );
}

#[rstest]
fn grep_filter_filters_lines() {
    let (mut env, state) = stdlib_env_with_state();
    state.reset_impure();
    env.add_template("grep", "{{ 'alpha\\nbeta\\n' | grep('beta') | trim }}")
        .expect("template");
    let template = env.get_template("grep").expect("get template");
    let rendered = template.render(context! {}).expect("render");
    assert_eq!(rendered, "beta");
    assert!(state.is_impure(), "grep should mark template impure");
}

#[rstest]
fn grep_filter_rejects_invalid_flags() {
    let (mut env, _state) = stdlib_env_with_state();
    env.add_template("grep_invalid", "{{ 'alpha' | grep('a', [1, 2, 3]) }}")
        .expect("template");
    let template = env.get_template("grep_invalid").expect("get template");
    let err = template
        .render(context! {})
        .expect_err("non-string flags should be rejected");
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    assert!(
        err.to_string().contains("grep flags must be strings"),
        "error should explain invalid flags: {err}",
    );
}

#[cfg(windows)]
fn compile_stub(dir: &Dir, root: &Utf8PathBuf, name: &str, source: &str) -> Utf8PathBuf {
    let source_name = format!("{name}.rs");
    dir.write(&source_name, source.as_bytes())
        .expect("write stub source");
    let src_path = root.join(&source_name);
    let exe_name = format!("{name}.exe");
    let exe_path = root.join(&exe_name);
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
    let status = Command::new(&rustc)
        .arg(src_path.as_std_path())
        .arg("-o")
        .arg(exe_path.as_std_path())
        .status()
        .expect("compile stub");
    assert!(status.success(), "failed to compile stub: {status:?}");
    exe_path
}

#[cfg(windows)]
#[fixture]
fn env_lock() -> EnvLock {
    EnvLock::acquire()
}

#[cfg(windows)]
const GREP_STUB: &str = concat!(
    "use std::io::{self, Read};\n",
    "fn main() {\n",
    "    let mut args: Vec<String> = std::env::args().skip(1).collect();\n",
    "    let pattern = args.pop().expect(\"pattern\");\n",
    "    let mut input = String::new();\n",
    "    io::stdin().read_to_string(&mut input).expect(\"stdin\");\n",
    "    if pattern == \"^line2\" && input.contains(\"line2\") {\n",
    "        print!(\"line2\\n\");\n",
    "    } else {\n",
    "        eprintln!(\"pattern:{pattern} input:{input}\", pattern = pattern, input = input);\n",
    "        std::process::exit(1);\n",
    "    }\n",
    "}\n",
);

#[cfg(windows)]
const ARGS_STUB: &str = concat!(
    "fn main() {\n",
    "    let mut args = std::env::args().skip(1);\n",
    "    if let Some(arg) = args.next() {\n",
    "        print!(\"{arg}\", arg = arg);\n",
    "    }\n",
    "}\n",
);

#[cfg(windows)]
#[rstest]
fn grep_on_windows_bypasses_shell(env_lock: EnvLock) {
    let _lock = env_lock;
    let temp = tempdir().expect("tempdir");
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8 temp");
    let dir = Dir::open_ambient_dir(&root, ambient_authority()).expect("open temp dir");
    compile_stub(&dir, &root, "grep", GREP_STUB);

    let mut path_value = OsString::from(root.as_str());
    path_value.push(";");
    path_value.push(std::env::var_os("PATH").unwrap_or_default());
    let _path = EnvVarGuard::set("PATH", &path_value);

    let (mut env, state) = stdlib_env_with_state();
    state.reset_impure();
    env.add_template(
        "grep_win",
        r#"{{ 'line1\nline2\n' | grep('^line2') | trim }}"#,
    )
    .expect("template");
    let template = env.get_template("grep_win").expect("get template");
    let rendered = template.render(context! {}).expect("render");
    assert_eq!(rendered, "line2");
    assert!(state.is_impure(), "grep should mark template impure");
}

#[cfg(windows)]
#[rstest]
fn shell_preserves_cmd_meta_characters(env_lock: EnvLock) {
    let _lock = env_lock;
    let temp = tempdir().expect("tempdir");
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8 temp");
    let dir = Dir::open_ambient_dir(&root, ambient_authority()).expect("open temp dir");
    let exe = compile_stub(&dir, &root, "echo_args", ARGS_STUB);

    let command = format!("\"{}\" \"literal %%^!\"", exe);
    let (mut env, state) = stdlib_env_with_state();
    state.reset_impure();
    env.add_template("shell_meta", "{{ '' | shell(cmd) }}")
        .expect("template");
    let template = env.get_template("shell_meta").expect("get template");
    let rendered = template
        .render(context!(cmd => command))
        .expect("render shell");
    assert_eq!(rendered.trim(), "literal %^!");
    assert!(
        state.is_impure(),
        "shell filter should mark template impure"
    );
}
