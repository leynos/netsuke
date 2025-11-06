//! Render-related stdlib step helpers used by Cucumber scenarios.
use crate::CliWorld;
use anyhow::{Context, Result};
use cap_std::{ambient_authority, fs_utf8::Dir};
use cucumber::when;
use minijinja::{Environment, context, value::Value};
use netsuke::stdlib::{self, StdlibConfig};

use super::types::{RelativePath, TemplateContent, TemplatePath};
use super::workspace::{ensure_workspace, resolve_template_path};

pub(crate) fn render_template_with_context(
    world: &mut CliWorld,
    template: &TemplateContent,
    ctx: Value,
) -> Result<()> {
    let root = ensure_workspace(world)?;
    let mut env = Environment::new();
    let workspace = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open stdlib workspace directory")?;
    let mut config = StdlibConfig::new(workspace).with_workspace_root_path(root.clone());
    if let Some(policy) = world.stdlib_policy.clone() {
        config = config.with_network_policy(policy);
    }
    if let Some(limit) = world.stdlib_fetch_max_bytes {
        config = config
            .with_fetch_max_response_bytes(limit)
            .context("configure stdlib fetch response limit")?;
    }
    if let Some(limit) = world.stdlib_command_max_output_bytes {
        config = config
            .with_command_max_output_bytes(limit)
            .context("configure stdlib command output limit")?;
    }
    if let Some(limit) = world.stdlib_command_stream_max_bytes {
        config = config
            .with_command_max_stream_bytes(limit)
            .context("configure stdlib command stream limit")?;
    }
    let state = stdlib::register_with_config(&mut env, config);
    state.reset_impure();
    world.stdlib_state = Some(state);
    let render = env.render_str(template.as_str(), ctx);
    match render {
        Ok(output) => {
            world.stdlib_output = Some(output);
            world.stdlib_error = None;
        }
        Err(err) => {
            world.stdlib_output = None;
            world.stdlib_error = Some(err.to_string());
        }
    }
    Ok(())
}

fn render_with_single_context(
    world: &mut CliWorld,
    template: &TemplateContent,
    key: &str,
    value: String,
) -> Result<()> {
    use minijinja::value::Value;
    use std::collections::BTreeMap;

    let ctx = Value::from_serialize(
        [(key, value)]
            .into_iter()
            .collect::<BTreeMap<&str, String>>(),
    );
    render_template_with_context(world, template, ctx)
}

fn render_template(
    world: &mut CliWorld,
    template: &TemplateContent,
    path: &TemplatePath,
) -> Result<()> {
    let ctx = context!(path => path.as_path().as_str());
    render_template_with_context(world, template, ctx)
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned capture arguments"
)]
#[when(regex = r#"^I render "(.+)" with stdlib path "(.+)"$"#)]
pub(crate) fn render_stdlib_template(
    world: &mut CliWorld,
    template_content: TemplateContent,
    relative_path: RelativePath,
) -> Result<()> {
    let root = ensure_workspace(world)?;
    let target = resolve_template_path(root.as_path(), relative_path);
    render_template(world, &template_content, &target)
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned capture arguments"
)]
#[when(regex = r#"^I render the stdlib template "(.+)"$"#)]
pub(crate) fn render_stdlib_template_without_path(
    world: &mut CliWorld,
    template_content: TemplateContent,
) -> Result<()> {
    render_template_with_context(world, &template_content, context! {})
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned capture arguments"
)]
#[when(regex = r#"^I render "(.+)" with stdlib url$"#)]
pub(crate) fn render_stdlib_template_with_url(
    world: &mut CliWorld,
    template_content: TemplateContent,
) -> Result<()> {
    let url = world
        .stdlib_url
        .clone()
        .context("expected stdlib HTTP server to be initialised")?;
    render_with_single_context(world, &template_content, "url", url)
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned capture arguments"
)]
#[when(regex = r#"^I render the stdlib template "(.+)" using the stdlib command helper$"#)]
pub(crate) fn render_stdlib_template_with_command(
    world: &mut CliWorld,
    template_content: TemplateContent,
) -> Result<()> {
    let command = world
        .stdlib_command
        .clone()
        .context("expected stdlib command helper to be compiled")?;
    render_with_single_context(world, &template_content, "cmd", command)
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned capture arguments"
)]
#[when(regex = r#"^I render the stdlib template "(.+)" using the stdlib text$"#)]
pub(crate) fn render_stdlib_template_with_text(
    world: &mut CliWorld,
    template_content: TemplateContent,
) -> Result<()> {
    let text = world
        .stdlib_text
        .clone()
        .context("expected stdlib template text to be configured")?;
    render_with_single_context(world, &template_content, "text", text)
}
