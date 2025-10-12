use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{Environment, ErrorKind, context, value::Value};
use rstest::rstest;
use tempfile::tempdir;
use test_support::command_helper::{compile_failure_helper, compile_uppercase_helper};

#[cfg(windows)]
use test_support::command_helper::compile_rust_helper;

use super::support::stdlib_env_with_state;

struct CommandFixture {
    _temp: tempfile::TempDir,
    env: Environment<'static>,
    state: netsuke::stdlib::StdlibState,
    command: String,
}

impl CommandFixture {
    fn new(compiler: impl Fn(&Dir, &Utf8PathBuf, &str) -> Utf8PathBuf, binary: &str) -> Self {
        let temp = tempdir().expect("tempdir");
        let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");
        let dir = Dir::open_ambient_dir(&root, ambient_authority()).expect("dir");
        let helper = compiler(&dir, &root, binary);
        let command = format!("\"{}\"", helper.as_str());
        let (env, state) = stdlib_env_with_state();
        state.reset_impure();
        Self {
            _temp: temp,
            env,
            state,
            command,
        }
    }

    fn env(&mut self) -> &mut Environment<'static> {
        &mut self.env
    }

    fn command(&self) -> &str {
        &self.command
    }

    fn state(&self) -> &netsuke::stdlib::StdlibState {
        &self.state
    }
}

type CommandCompiler = fn(&Dir, &Utf8PathBuf, &str) -> Utf8PathBuf;

enum ShellExpectation {
    Success(&'static str),
    Failure { substrings: &'static [&'static str] },
}

#[cfg(windows)]
use {
    super::support::{EnvLock, EnvVarGuard},
    rstest::fixture,
    std::ffi::OsString,
};

#[rstest]
#[case::uppercase(
    compile_uppercase_helper,
    "cmd_upper",
    "shell_upper",
    "{{ 'hello' | shell(cmd) | trim }}",
    ShellExpectation::Success("HELLO")
)]
#[case::failure(
    compile_failure_helper,
    "cmd_fail",
    "shell_fail",
    "{{ 'data' | shell(cmd) }}",
    ShellExpectation::Failure {
        substrings: &["command", "exited"],
    },
)]
fn shell_filter_behaviour(
    #[case] compiler: CommandCompiler,
    #[case] binary: &'static str,
    #[case] template_name: &'static str,
    #[case] template_src: &'static str,
    #[case] expectation: ShellExpectation,
) {
    let mut fixture = CommandFixture::new(compiler, binary);
    {
        let env = fixture.env();
        env.add_template(template_name, template_src)
            .expect("template");
    }
    let command = fixture.command().to_owned();
    let template = {
        let env = fixture.env();
        env.get_template(template_name).expect("get template")
    };

    match expectation {
        ShellExpectation::Success(expected) => {
            let rendered = template
                .render(context!(cmd => command.clone()))
                .expect("render shell");
            assert_eq!(rendered, expected);
        }
        ShellExpectation::Failure { substrings } => {
            let err = template
                .render(context!(cmd => command.clone()))
                .expect_err("shell should propagate failures");
            assert_eq!(err.kind(), ErrorKind::InvalidOperation);
            let message = err.to_string();
            for needle in substrings {
                assert!(
                    message.contains(needle),
                    "error should mention {needle}: {message}",
                );
            }
        }
    }

    assert!(
        fixture.state().is_impure(),
        "shell filter should mark template impure",
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

#[cfg(unix)]
#[rstest]
fn shell_filter_tolerates_commands_that_close_stdin() {
    let (mut env, state) = stdlib_env_with_state();
    state.reset_impure();
    env.add_template(
        "shell_head",
        "{{ 'alpha\\nbeta\\n' | shell('head -n1') | trim }}",
    )
    .expect("template");
    let template = env.get_template("shell_head").expect("get template");

    let rendered = template
        .render(context! {})
        .expect("head should exit successfully");
    assert_eq!(rendered, "alpha");
    assert!(
        state.is_impure(),
        "head command should mark template impure"
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

fn empty_context() -> Value {
    context! {}
}

fn shell_context() -> Value {
    context!(cmd => "echo ignored")
}

#[rstest]
#[case(
    "shell_undefined",
    "{{ missing | shell(cmd) }}",
    shell_context as fn() -> Value,
    "shell filter should mark template impure",
)]
#[case(
    "grep_undefined",
    "{{ missing | grep('pattern') }}",
    empty_context as fn() -> Value,
    "grep filter should mark template impure",
)]
fn filters_reject_undefined_input(
    #[case] name: &str,
    #[case] template_src: &str,
    #[case] context_fn: fn() -> Value,
    #[case] impure_message: &str,
) {
    let (mut env, state) = stdlib_env_with_state();
    state.reset_impure();
    env.add_template(name, template_src).expect("template");
    let template = env.get_template(name).expect("get template");
    let err = template
        .render(context_fn())
        .expect_err("filter should reject undefined input");
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    assert!(
        err.to_string().contains("input value is undefined"),
        "error should mention undefined input: {err}",
    );
    assert!(state.is_impure(), "{impure_message}");
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
    compile_rust_helper(&dir, &root, "grep", GREP_STUB);

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
    let exe = compile_rust_helper(&dir, &root, "echo_args", ARGS_STUB);

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
