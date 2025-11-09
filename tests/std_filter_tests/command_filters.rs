use anyhow::{anyhow, bail, ensure, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{context, value::Value, Environment, ErrorKind};
use rstest::rstest;
use tempfile::tempdir;
use test_support::command_helper::{
    compile_failure_helper,
    compile_large_output_helper,
    compile_uppercase_helper,
};
use std::fs;

#[cfg(windows)]
use test_support::command_helper::compile_rust_helper;

use super::support::fallible;
use netsuke::stdlib::StdlibConfig;

struct CommandFixture {
    _temp: tempfile::TempDir,
    env: Environment<'static>,
    state: netsuke::stdlib::StdlibState,
    command: String,
}

impl CommandFixture {
    fn with_config(
        compiler: CommandCompiler,
        binary: &str,
        config: StdlibConfig,
    ) -> Result<Self> {
        let temp = tempdir().context("create command fixture tempdir")?;
        let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
            .map_err(|path| anyhow!("command fixture root is not valid UTF-8: {path:?}"))?;
        let dir = Dir::open_ambient_dir(&root, ambient_authority())
            .context("open command fixture directory")?;
        let helper = compiler(&dir, &root, binary)?;
        let command = format!("\"{}\"", helper.as_str());
        let (env, mut state) = fallible::stdlib_env_with_config(config)?;
        state.reset_impure();
        Ok(Self {
            _temp: temp,
            env,
            state,
            command,
        })
    }

    fn new(
        compiler: CommandCompiler,
        binary: &str,
    ) -> Result<Self> {
        Self::with_config(compiler, binary, StdlibConfig::default())
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

const STREAM_LINE_COUNT: usize = 32_768;

fn streaming_match_payload() -> String {
    let mut text = String::with_capacity(STREAM_LINE_COUNT * 6);
    for _ in 0..STREAM_LINE_COUNT {
        text.push_str("match\n");
    }
    text
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
fn shell_filter_rejects_empty_command() -> Result<()> {
    let (mut env, mut state) = fallible::stdlib_env_with_state()?;
    state.reset_impure();
    fallible::register_template(&mut env, "shell_empty", "{{ 'hi' | shell('   ') }}")?;
    let template = env
        .get_template("shell_empty")
        .context("fetch template 'shell_empty'")?;
    let err = match template.render(context! {}) {
        Ok(output) => bail!("expected shell to reject blank commands but rendered {output}"),
        Err(err) => err,
    };
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "shell should report InvalidOperation but was {:?}",
        err.kind()
    );
    ensure!(
        err.to_string().contains("requires a non-empty command"),
        "error should mention validation message: {err}"
    );
    ensure!(state.is_impure(), "shell should still mark template impure");
    Ok(())
}

#[rstest]
fn shell_filter_enforces_output_limit() -> Result<()> {
    let config = StdlibConfig::default()
        .with_command_max_output_bytes(1024)?;
    let mut fixture =
        CommandFixture::with_config(compile_large_output_helper, "cmd_large", config)?;
    {
        let env = fixture.env();
        fallible::register_template(env, "shell_large", "{{ '' | shell(cmd) }}")?;
    }
    let command = fixture.command().to_owned();
    let template = {
        let env = fixture.env();
        env.get_template("shell_large")
            .context("fetch template 'shell_large'")?
    };
    let err = match template.render(context!(cmd => command)) {
        Ok(output) => bail!("expected shell output limit error but rendered {output}"),
        Err(err) => err,
    };
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "shell output limit should report InvalidOperation but was {:?}",
        err.kind()
    );
    ensure!(
        err.to_string().contains("stdout capture limit of 1024 bytes"),
        "limit error should mention configured budget: {err}"
    );
    ensure!(fixture.state().is_impure(), "limit error should mark template impure");
    Ok(())
}

#[rstest]
fn shell_filter_streams_to_tempfiles() -> Result<()> {
    let config = StdlibConfig::default()
        .with_command_max_output_bytes(512)?
        .with_command_max_stream_bytes(200_000)?;
    let mut fixture =
        CommandFixture::with_config(compile_large_output_helper, "cmd_stream", config)?;
    {
        let env = fixture.env();
        fallible::register_template(
            env,
            "shell_stream",
            "{{ '' | shell(cmd, {'mode': 'tempfile'}) }}",
        )?;
    }
    let command = fixture.command().to_owned();
    let template = {
        let env = fixture.env();
        env.get_template("shell_stream")
            .context("fetch template 'shell_stream'")?
    };
    let rendered = template
        .render(context!(cmd => command))
        .context("render shell streaming template")?;
    ensure!(fixture.state().is_impure(), "streaming should mark template impure");
    let path = Utf8Path::new(&rendered);
    let data = fs::read(path.as_std_path()).with_context(|| {
        format!("read streamed output from {}", path.as_str())
    })?;
    ensure!(
        data.len() >= 65_000,
        "expected streamed output to contain command data"
    );
    ensure!(
        data.iter().all(|byte| *byte == b'x'),
        "streamed file should contain the helper payload"
    );
    Ok(())
}

#[rstest]
fn shell_streaming_honours_size_limit() -> Result<()> {
    let config = StdlibConfig::default()
        .with_command_max_output_bytes(256)?
        .with_command_max_stream_bytes(1024)?;
    let mut fixture =
        CommandFixture::with_config(compile_large_output_helper, "cmd_stream_limit", config)?;
    {
        let env = fixture.env();
        fallible::register_template(
            env,
            "shell_stream_limit",
            "{{ '' | shell(cmd, {'mode': 'tempfile'}) }}",
        )?;
    }
    let command = fixture.command().to_owned();
    let template = {
        let env = fixture.env();
        env.get_template("shell_stream_limit")
            .context("fetch template 'shell_stream_limit'")?
    };
    let err = match template.render(context!(cmd => command)) {
        Ok(output) => bail!("expected streaming limit error but rendered {output}"),
        Err(err) => err,
    };
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "streaming limit should report InvalidOperation but was {:?}",
        err.kind()
    );
    ensure!(
        err.to_string().contains("stdout streaming limit of 1024 bytes"),
        "streaming limit error should mention configured budget: {err}"
    );
    ensure!(fixture.state().is_impure(), "streaming limit should mark template impure");
    Ok(())
}

#[cfg(not(windows))]
#[rstest]
fn grep_filter_streams_to_tempfiles() -> Result<()> {
    let config = StdlibConfig::default()
        .with_command_max_output_bytes(512)?
        .with_command_max_stream_bytes(200_000)?;
    let (mut env, mut state) = fallible::stdlib_env_with_config(config)?;
    state.reset_impure();
    fallible::register_template(
        &mut env,
        "grep_stream",
        "{{ text | grep('match', none, {'mode': 'tempfile'}) }}",
    )?;
    let template = env
        .get_template("grep_stream")
        .context("fetch template 'grep_stream'")?;
    let payload = streaming_match_payload();
    let rendered = template
        .render(context!(text => payload.clone()))
        .context("render grep streaming template")?;
    ensure!(state.is_impure(), "grep streaming should mark template impure");
    let path = Utf8Path::new(rendered.as_str());
    let metadata = fs::metadata(path.as_std_path())
        .with_context(|| format!("stat streamed grep output {}", path))?;
    ensure!(
        metadata.len() >= payload.len() as u64,
        "streamed grep output should retain payload size"
    );
    let contents = fs::read_to_string(path.as_std_path())
        .with_context(|| format!("read streamed grep output {}", path))?;
    ensure!(
        contents == payload,
        "streamed grep file should contain the helper payload"
    );
    Ok(())
}

#[cfg(not(windows))]
#[rstest]
fn grep_filter_enforces_output_limit() -> Result<()> {
    let config = StdlibConfig::default()
        .with_command_max_output_bytes(1024)?;
    let (mut env, mut state) = fallible::stdlib_env_with_config(config)?;
    state.reset_impure();
    let long_text = "x".repeat(2_500);
    fallible::register_template(
        &mut env,
        "grep_limit",
        "{{ text | grep('x') }}",
    )?;
    let template = env
        .get_template("grep_limit")
        .context("fetch template 'grep_limit'")?;
    let err = match template.render(context!(text => long_text)) {
        Ok(output) => bail!("expected grep output limit error but rendered {output}"),
        Err(err) => err,
    };
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "grep limit should report InvalidOperation but was {:?}",
        err.kind()
    );
    ensure!(
        err.to_string().contains("stdout capture limit of 1024 bytes"),
        "grep error should mention configured limit: {err}"
    );
    ensure!(state.is_impure(), "grep limit should mark template impure");
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

#[rstest]
fn grep_filter_rejects_empty_pattern() -> Result<()> {
    let (mut env, mut state) = fallible::stdlib_env_with_state()?;
    state.reset_impure();
    fallible::register_template(&mut env, "grep_empty", "{{ 'alpha' | grep('') }}")?;
    let template = env
        .get_template("grep_empty")
        .context("fetch template 'grep_empty'")?;
    let err = match template.render(context! {}) {
        Ok(output) => bail!("expected grep to reject empty patterns but rendered {output}"),
        Err(err) => err,
    };
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "grep should report InvalidOperation but was {:?}",
        err.kind()
    );
    ensure!(
        err.to_string().contains("requires a search pattern"),
        "error message should mention missing pattern: {err}"
    );
    ensure!(state.is_impure(), "grep should mark template impure");
    Ok(())
}

#[cfg(not(windows))]
#[rstest]
fn grep_filter_handles_patterns_with_spaces() -> Result<()> {
    let (mut env, mut state) = fallible::stdlib_env_with_state()?;
    state.reset_impure();
    fallible::register_template(
        &mut env,
        "grep_space_pattern",
        "{{ text | grep('needs space') | trim }}",
    )?;
    let template = env
        .get_template("grep_space_pattern")
        .context("fetch template 'grep_space_pattern'")?;
    let rendered = template
        .render(context!(text => "needs space\nother"))
        .context("render grep space template")?;
    ensure!(rendered == "needs space", "grep should match spaced pattern");
    ensure!(state.is_impure(), "grep should mark template impure");
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
const GREP_STREAM_STUB: &str = concat!(
    "use std::io::{self, Write};\n",
    "fn main() {\n",
    "    let pattern = std::env::args().last().expect(\"pattern\");\n",
    "    if pattern != \"match\" {\n",
    "        std::process::exit(1);\n",
    "    }\n",
    "    let mut out = io::stdout();\n",
    "    let line = b\"match\\n\";\n",
    "    for _ in 0..32_768 {\n",
    "        out.write_all(line).expect(\"stdout\");\n",
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
fn grep_streams_large_output_on_windows(env_lock: EnvLock) -> Result<()> {
    let _lock = env_lock;
    let temp = tempdir().context("create windows grep stream tempdir")?;
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow!("windows grep root is not valid UTF-8: {path:?}"))?;
    let dir = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open windows grep stream temp dir")?;
    compile_rust_helper(&dir, &root, "grep", GREP_STREAM_STUB)
        .context("compile streaming grep helper for windows tests")?;

    let mut path_value = OsString::from(root.as_str());
    path_value.push(";");
    path_value.push(std::env::var_os("PATH").unwrap_or_default());
    let _path = EnvVarGuard::set("PATH", &path_value);

    let config = StdlibConfig::default()
        .with_command_max_output_bytes(512)?
        .with_command_max_stream_bytes(200_000)?;
    let (mut env, mut state) = fallible::stdlib_env_with_config(config)?;
    state.reset_impure();
    fallible::register_template(
        &mut env,
        "grep_win_stream",
        "{{ text | grep('match', none, {'mode': 'tempfile'}) }}",
    )?;
    let template = env
        .get_template("grep_win_stream")
        .context("fetch template 'grep_win_stream'")?;
    let payload = streaming_match_payload();
    let rendered = template
        .render(context!(text => payload.clone()))
        .context("render windows grep streaming template")?;
    ensure!(state.is_impure(), "grep streaming should mark template impure");
    let path = Utf8Path::new(rendered.as_str());
    let metadata = fs::metadata(path.as_std_path())
        .with_context(|| format!("stat streamed windows grep output {}", path))?;
    ensure!(
        metadata.len() >= payload.len() as u64,
        "streamed grep output should retain payload size"
    );
    let contents = fs::read_to_string(path.as_std_path())
        .with_context(|| format!("read streamed windows grep output {}", path))?;
    ensure!(
        contents == payload,
        "streamed grep file should contain the helper payload"
    );
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
