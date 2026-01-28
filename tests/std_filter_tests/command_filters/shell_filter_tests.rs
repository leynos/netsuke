//! Shell filter behaviour tests.

use anyhow::{bail, ensure, Context, Result};
use minijinja::{context, ErrorKind};
use rstest::rstest;
use std::fs;
use test_support::command_helper::{
    compile_failure_helper,
    compile_large_output_helper,
    compile_uppercase_helper,
};

use super::{
    CommandCompiler, CommandFixture, ShellExpectation, StdlibConfig, fallible,
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
        err.to_string().contains("exceeded capture stdout limit of 1024 bytes"),
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
    let path = camino::Utf8Path::new(&rendered);
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
        err.to_string().contains("exceeded streaming stdout limit of 1024 bytes"),
        "streaming limit error should mention configured budget: {err}"
    );
    ensure!(fixture.state().is_impure(), "streaming limit should mark template impure");
    Ok(())
}
