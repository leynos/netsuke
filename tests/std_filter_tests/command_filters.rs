use anyhow::{anyhow, bail, ensure, Context, Result};
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{context, value::Value, Environment, ErrorKind};
use rstest::rstest;
use tempfile::tempdir;
use test_support::command_helper::{compile_failure_helper, compile_uppercase_helper};

#[cfg(windows)]
use test_support::command_helper::compile_rust_helper;

use super::support::fallible;

struct CommandFixture {
    _temp: tempfile::TempDir,
    env: Environment<'static>,
    state: netsuke::stdlib::StdlibState,
    command: String,
}

impl CommandFixture {
    fn new(
        compiler: CommandCompiler,
        binary: &str,
    ) -> Result<Self> {
        let temp = tempdir().context("create command fixture tempdir")?;
        let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
            .map_err(|path| anyhow!("command fixture root is not valid UTF-8: {path:?}"))?;
        let dir = Dir::open_ambient_dir(&root, ambient_authority())
            .context("open command fixture directory")?;
        let helper = compiler(&dir, &root, binary)?;
        let command = format!("\"{}\"", helper.as_str());
        let (env, mut state) = fallible::stdlib_env_with_state()?;
        state.reset_impure();
        Ok(Self {
            _temp: temp,
            env,
            state,
            command,
        })
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

type CommandCompiler = fn(&Dir, &Utf8PathBuf, &str) -> Result<Utf8PathBuf>;

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
) -> Result<()> {
    let mut fixture = CommandFixture::new(compiler, binary)?;
    {
        let env = fixture.env();
        fallible::register_template(env, template_name, template_src)?;
    }
    let command = fixture.command().to_owned();
    let template = {
        let env = fixture.env();
        env.get_template(template_name)
            .with_context(|| format!("fetch template '{template_name}'"))?
    };

    match expectation {
        ShellExpectation::Success(expected) => {
            let rendered = template
                .render(context!(cmd => command.clone()))
                .context("render shell template")?;
            ensure!(rendered == expected, "expected '{expected}' but rendered {rendered}");
        }
        ShellExpectation::Failure { substrings } => {
            let err = match template.render(context!(cmd => command.clone())) {
                Ok(output) => bail!(
                    "expected shell to propagate failures but rendered {output}"
                ),
                Err(err) => err,
            };
            ensure!(
                err.kind() == ErrorKind::InvalidOperation,
                "shell should report InvalidOperation but was {:?}",
                err.kind()
            );
            let message = err.to_string();
            for needle in substrings {
                ensure!(
                    message.contains(needle),
                    "error should mention {needle}: {message}"
                );
            }
        }
    }

    ensure!(
        fixture.state().is_impure(),
        "shell filter should mark template impure"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn shell_filter_times_out_long_commands() -> Result<()> {
    let (mut env, mut state) = fallible::stdlib_env_with_state()?;
    state.reset_impure();
    fallible::register_template(&mut env, "shell_timeout", "{{ '' | shell('sleep 10') }}")?;
    let template = env
        .get_template("shell_timeout")
        .context("fetch template 'shell_timeout'")?;
    let err = match template.render(context! {}) {
        Ok(output) => bail!(
            "expected shell timeout but command completed with output {output}"
        ),
        Err(err) => err,
    };
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "shell timeout should report InvalidOperation but was {:?}",
        err.kind()
    );
    ensure!(state.is_impure(), "timeout should mark template impure");
    ensure!(
        err.to_string().contains("timed out"),
        "timeout error should mention duration: {err}"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn shell_filter_tolerates_commands_that_close_stdin() -> Result<()> {
    let (mut env, mut state) = fallible::stdlib_env_with_state()?;
    state.reset_impure();
    fallible::register_template(
        &mut env,
        "shell_head",
        "{{ 'alpha\\nbeta\\n' | shell('head -n1') | trim }}",
    )?;
    let template = env
        .get_template("shell_head")
        .context("fetch template 'shell_head'")?;

    let rendered = template
        .render(context! {})
        .context("render shell head template")?;
    ensure!(rendered == "alpha", "expected 'alpha' but rendered {rendered}");
    ensure!(
        state.is_impure(),
        "head command should mark template impure"
    );
    Ok(())
}

#[rstest]
fn grep_filter_filters_lines() -> Result<()> {
    let (mut env, mut state) = fallible::stdlib_env_with_state()?;
    state.reset_impure();
    fallible::register_template(&mut env, "grep", "{{ 'alpha\\nbeta\\n' | grep('beta') | trim }}")?;
    let template = env
        .get_template("grep")
        .context("fetch template 'grep'")?;
    let rendered = template
        .render(context! {})
        .context("render grep template")?;
    ensure!(rendered == "beta", "expected 'beta' but rendered {rendered}");
    ensure!(state.is_impure(), "grep should mark template impure");
    Ok(())
}

#[rstest]
fn grep_filter_rejects_invalid_flags() -> Result<()> {
    let (mut env, _state) = fallible::stdlib_env_with_state()?;
    fallible::register_template(&mut env, "grep_invalid", "{{ 'alpha' | grep('a', [1, 2, 3]) }}")?;
    let template = env
        .get_template("grep_invalid")
        .context("fetch template 'grep_invalid'")?;
    let err = match template.render(context! {}) {
        Ok(output) => bail!(
            "expected grep to reject non-string flags but rendered {output}"
        ),
        Err(err) => err,
    };
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "grep should report InvalidOperation for invalid flags but was {:?}",
        err.kind()
    );
    ensure!(
        err.to_string().contains("grep flags must be strings"),
        "error should explain invalid flags: {err}"
    );
    Ok(())
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
) -> Result<()> {
    let (mut env, mut state) = fallible::stdlib_env_with_state()?;
    state.reset_impure();
    fallible::register_template(&mut env, name, template_src)?;
    let template = env
        .get_template(name)
        .with_context(|| format!("fetch template '{name}'"))?;
    let err = match template.render(context_fn()) {
        Ok(output) => bail!(
            "expected filter to reject undefined input but rendered {output}"
        ),
        Err(err) => err,
    };
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "filter should report InvalidOperation for undefined input but was {:?}",
        err.kind()
    );
    ensure!(
        err.to_string().contains("input value is undefined"),
        "error should mention undefined input: {err}"
    );
    ensure!(state.is_impure(), "{impure_message}");
    Ok(())
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
fn grep_on_windows_bypasses_shell(env_lock: EnvLock) -> Result<()> {
    let _lock = env_lock;
    let temp = tempdir().context("create windows grep tempdir")?;
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow!("windows grep root is not valid UTF-8: {path:?}"))?;
    let dir = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open windows grep temp dir")?;
    compile_rust_helper(&dir, &root, "grep", GREP_STUB)
        .context("compile grep helper for windows tests")?;

    let mut path_value = OsString::from(root.as_str());
    path_value.push(";");
    path_value.push(std::env::var_os("PATH").unwrap_or_default());
    let _path = EnvVarGuard::set("PATH", &path_value);

    let (mut env, mut state) = fallible::stdlib_env_with_state()?;
    state.reset_impure();
    fallible::register_template(
        &mut env,
        "grep_win",
        r#"{{ 'line1
line2
' | grep('^line2') | trim }}"#,
    )?;
    let template = env
        .get_template("grep_win")
        .context("fetch template 'grep_win'")?;
    let rendered = template
        .render(context! {})
        .context("render windows grep template")?;
    ensure!(rendered == "line2", "expected 'line2' but rendered {rendered}");
    ensure!(state.is_impure(), "grep should mark template impure");
    Ok(())
}

#[cfg(windows)]
#[rstest]
fn shell_preserves_cmd_meta_characters(env_lock: EnvLock) -> Result<()> {
    let _lock = env_lock;
    let temp = tempdir().context("create windows shell tempdir")?;
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow!("windows shell root is not valid UTF-8: {path:?}"))?;
    let dir = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open windows shell temp dir")?;
    let exe = compile_rust_helper(&dir, &root, "echo_args", ARGS_STUB)
        .context("compile echo_args helper for windows tests")?;

    let command = format!("\"{}\" \"literal %%^!\"", exe);
    let (mut env, mut state) = fallible::stdlib_env_with_state()?;
    state.reset_impure();
    fallible::register_template(&mut env, "shell_meta", "{{ '' | shell(cmd) }}")?;
    let template = env
        .get_template("shell_meta")
        .context("fetch template 'shell_meta'")?;
    let rendered = template
        .render(context!(cmd => command))
        .context("render shell meta template")?;
    ensure!(rendered.trim() == "literal %^!", "expected literal %^! but rendered {rendered}");
    ensure!(
        state.is_impure(),
        "shell filter should mark template impure"
    );
    Ok(())
}
