//! Grep filter behaviour tests.

use anyhow::{bail, ensure, Context, Result};
use minijinja::{context, ErrorKind};
use rstest::rstest;
use std::fs;

use super::{StdlibConfig, fallible, streaming_match_payload};

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
    let path = camino::Utf8Path::new(rendered.as_str());
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
        err.to_string().contains("exceeded capture stdout limit of 1024 bytes"),
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
        err.to_string().contains("Grep flags must be strings"),
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
        err.to_string().contains("Grep pattern must not be empty"),
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
