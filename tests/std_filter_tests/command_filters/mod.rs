//! Command filter integration tests for stdlib shell and grep filters.

use anyhow::{anyhow, bail, ensure, Context, Result};
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{context, value::Value, Environment, ErrorKind};
use rstest::rstest;
use tempfile::tempdir;

pub(super) use super::support::fallible;
pub(super) use netsuke::stdlib::StdlibConfig;

pub(super) struct CommandFixture {
    _temp: tempfile::TempDir,
    env: Environment<'static>,
    state: netsuke::stdlib::StdlibState,
    command: String,
}

impl CommandFixture {
    pub(super) fn with_config(
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

    pub(super) fn new(compiler: CommandCompiler, binary: &str) -> Result<Self> {
        Self::with_config(compiler, binary, StdlibConfig::default())
    }

    pub(super) fn env(&mut self) -> &mut Environment<'static> {
        &mut self.env
    }

    pub(super) fn command(&self) -> &str {
        &self.command
    }

    pub(super) fn state(&self) -> &netsuke::stdlib::StdlibState {
        &self.state
    }
}

pub(super) type CommandCompiler = fn(&Dir, &Utf8PathBuf, &str) -> Result<Utf8PathBuf>;

pub(super) enum ShellExpectation {
    Success(&'static str),
    Failure {
        substrings: &'static [&'static str],
    },
}

const STREAM_LINE_COUNT: usize = 32_768;

pub(super) fn streaming_match_payload() -> String {
    let mut text = String::with_capacity(STREAM_LINE_COUNT * 6);
    for _ in 0..STREAM_LINE_COUNT {
        text.push_str("match\n");
    }
    text
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

mod shell_filter_tests;
mod grep_filter_tests;
#[cfg(windows)]
mod windows_filter_tests;
