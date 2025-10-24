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
    let config = StdlibConfig::new(workspace);
    let state = stdlib::register_with_config(&mut env, config);
    state.reset_impure();
    world.stdlib_state = Some(state.clone());
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

fn render_template(
    world: &mut CliWorld,
    template: &TemplateContent,
    path: &TemplatePath,
) -> Result<()> {
    let ctx = context!(path => path.as_path().as_str());
    render_template_with_context(world, template, ctx)
}

#[when(regex = r#"^I render "(.+)" with stdlib path "(.+)"$"#)]
pub(crate) fn render_stdlib_template(
    world: &mut CliWorld,
    template_source: String,
    raw_path: String,
) -> Result<()> {
    let root = ensure_workspace(world)?;
    let template_content = TemplateContent::from(template_source);
    let relative_path = RelativePath::from(raw_path);
    let target = resolve_template_path(root.as_path(), &relative_path);
    render_template(world, &template_content, &target)
}

#[when(regex = r#"^I render the stdlib template "(.+)"$"#)]
pub(crate) fn render_stdlib_template_without_path(
    world: &mut CliWorld,
    template_source: String,
) -> Result<()> {
    let template_content = TemplateContent::from(template_source);
    render_template_with_context(world, &template_content, context! {})
}

#[when(regex = r#"^I render "(.+)" with stdlib url$"#)]
pub(crate) fn render_stdlib_template_with_url(
    world: &mut CliWorld,
    template_source: String,
) -> Result<()> {
    let url = world
        .stdlib_url
        .clone()
        .context("expected stdlib HTTP server to be initialised")?;
    let template_content = TemplateContent::from(template_source);
    render_template_with_context(world, &template_content, context!(url => url))
}

#[when(regex = r#"^I render the stdlib template "(.+)" using the stdlib command helper$"#)]
pub(crate) fn render_stdlib_template_with_command(
    world: &mut CliWorld,
    template_source: String,
) -> Result<()> {
    let command = world
        .stdlib_command
        .clone()
        .context("expected stdlib command helper to be compiled")?;
    let template_content = TemplateContent::from(template_source);
    render_template_with_context(world, &template_content, context!(cmd => command))
}
