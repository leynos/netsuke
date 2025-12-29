//! Render-related stdlib step helpers used by BDD scenarios.

use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use crate::bdd::types::{ContextKey, ContextValue, TemplateContent};
use anyhow::{Context, Result};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{Environment, context, value::Value};
use netsuke::stdlib::{self, NetworkPolicy, StdlibConfig};
use rstest_bdd_macros::when;

use super::types::TemplatePath;
use super::workspace::{ensure_workspace, resolve_template_path};

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Configuration values extracted from the test world for stdlib rendering.
struct RenderConfig {
    policy: Option<NetworkPolicy>,
    fetch_max_bytes: Option<u64>,
    command_max_output_bytes: Option<u64>,
    command_stream_max_bytes: Option<u64>,
}

fn extract_render_config(world: &TestWorld) -> RenderConfig {
    RenderConfig {
        policy: world.stdlib_policy.with_ref(Clone::clone),
        fetch_max_bytes: world.stdlib_fetch_max_bytes.get(),
        command_max_output_bytes: world.stdlib_command_max_output_bytes.get(),
        command_stream_max_bytes: world.stdlib_command_stream_max_bytes.get(),
    }
}

pub(crate) fn render_template_with_context(
    world: &TestWorld,
    template: &TemplateContent,
    ctx: Value,
) -> Result<()> {
    let root = ensure_workspace(world)?;
    let mut env = Environment::new();
    let workspace = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open stdlib workspace directory")?;
    let mut config = StdlibConfig::new(workspace)?.with_workspace_root_path(root.clone())?;

    // Extract config from world before applying
    let render_cfg = extract_render_config(world);

    if let Some(policy) = render_cfg.policy {
        config = config.with_network_policy(policy);
    }
    if let Some(limit) = render_cfg.fetch_max_bytes {
        config = config
            .with_fetch_max_response_bytes(limit)
            .context("configure stdlib fetch response limit")?;
    }
    if let Some(limit) = render_cfg.command_max_output_bytes {
        config = config
            .with_command_max_output_bytes(limit)
            .context("configure stdlib command output limit")?;
    }
    if let Some(limit) = render_cfg.command_stream_max_bytes {
        config = config
            .with_command_max_stream_bytes(limit)
            .context("configure stdlib command stream limit")?;
    }
    if let Some(path) = world.stdlib_path_override.borrow().as_ref() {
        config = config.with_path_override(path.clone());
    }

    let state = stdlib::register_with_config(&mut env, config)?;
    state.reset_impure();

    let render = env.render_str(template.as_str(), ctx);
    world.stdlib_state.set_value(state);
    match render {
        Ok(output) => {
            world.stdlib_output.set(output);
            world.stdlib_error.clear();
        }
        Err(err) => {
            world.stdlib_output.clear();
            world.stdlib_error.set(err.to_string());
        }
    }
    Ok(())
}

fn render_with_single_context(
    world: &TestWorld,
    template: &TemplateContent,
    key: &ContextKey,
    value: ContextValue,
) -> Result<()> {
    use std::collections::BTreeMap;

    let ctx = Value::from_serialize(
        [(key.as_str(), value.into_string())]
            .into_iter()
            .collect::<BTreeMap<&str, String>>(),
    );
    render_template_with_context(world, template, ctx)
}

fn render_template(
    world: &TestWorld,
    template: &TemplateContent,
    path: &TemplatePath,
) -> Result<()> {
    let ctx = context!(path => path.as_path().as_str());
    render_template_with_context(world, template, ctx)
}

/// Source of context values for template rendering.
///
/// Each variant corresponds to a world field that provides a context value
/// for template rendering, along with its associated context key.
#[derive(Copy, Clone)]
enum ContextSource {
    Url,
    Command,
    Text,
}

impl ContextSource {
    /// Return the context key for this source.
    const fn key(self) -> &'static str {
        match self {
            Self::Url => "url",
            Self::Command => "cmd",
            Self::Text => "text",
        }
    }

    /// Retrieve the value from the world for this context source.
    fn get_value(self, world: &TestWorld) -> Result<String> {
        match self {
            Self::Url => world
                .stdlib_url
                .get()
                .context("expected stdlib HTTP server to be initialised"),
            Self::Command => world
                .stdlib_command
                .get()
                .context("expected stdlib command helper to be compiled"),
            Self::Text => world
                .stdlib_text
                .get()
                .context("expected stdlib template text to be configured"),
        }
    }
}

/// Render a template using a context value from the specified source.
fn render_with_context_source(
    world: &TestWorld,
    template: &TemplateContent,
    source: ContextSource,
) -> Result<()> {
    let value = source.get_value(world)?;
    render_with_single_context(
        world,
        template,
        &ContextKey::new(source.key()),
        ContextValue::new(value),
    )
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macro generates wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[when("I render template {template:string} at stdlib path {path:string}")]
pub(crate) fn render_stdlib_template(world: &TestWorld, template: &str, path: &str) -> Result<()> {
    let template = TemplateContent::new(template);
    let root = ensure_workspace(world)?;
    let target = resolve_template_path(root.as_path(), TemplatePath::new(path));
    render_template(world, &template, &target)
}

#[expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macro generates wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[when("I render the stdlib template {template:string} without context")]
pub(crate) fn render_stdlib_template_without_path(world: &TestWorld, template: &str) -> Result<()> {
    let template = TemplateContent::new(template);
    render_template_with_context(world, &template, context! {})
}

#[expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macro generates wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[when("I render template {template:string} with stdlib url")]
pub(crate) fn render_stdlib_template_with_url(world: &TestWorld, template: &str) -> Result<()> {
    let template = TemplateContent::new(template);
    render_with_context_source(world, &template, ContextSource::Url)
}

#[expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macro generates wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[when("I render the stdlib template {template:string} using the stdlib command helper")]
pub(crate) fn render_stdlib_template_with_command(world: &TestWorld, template: &str) -> Result<()> {
    let template = TemplateContent::new(template);
    render_with_context_source(world, &template, ContextSource::Command)
}

#[expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macro generates wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[when("I render the stdlib template {template:string} using the stdlib text")]
pub(crate) fn render_stdlib_template_with_text(world: &TestWorld, template: &str) -> Result<()> {
    let template = TemplateContent::new(template);
    render_with_context_source(world, &template, ContextSource::Text)
}
