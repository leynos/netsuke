//! Render-related stdlib step helpers used by BDD scenarios.

use crate::bdd::fixtures::{RefCellOptionExt, strip_quotes, with_world};
use anyhow::{Context, Result};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{Environment, context, value::Value};
use netsuke::stdlib::{self, NetworkPolicy, StdlibConfig};
use rstest_bdd_macros::when;

use super::types::TemplatePath;
use super::workspace::{ensure_workspace, resolve_template_path};

/// Newtype wrapper for template content strings.
///
/// Distinguishes template source from other string parameters in rendering
/// functions, improving type safety and API clarity.
pub(crate) struct TemplateContent(String);

impl TemplateContent {
    pub(crate) fn new(content: impl Into<String>) -> Self {
        Self(content.into())
    }
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for TemplateContent {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl AsRef<str> for TemplateContent {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Newtype wrapper for context key strings.
///
/// Distinguishes context variable names from other string parameters,
/// preventing accidental misuse of keys and values.
pub(crate) struct ContextKey(String);

impl ContextKey {
    pub(crate) fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// Newtype wrapper for context value strings.
///
/// Distinguishes context variable values from other string parameters,
/// ensuring type safety when constructing template rendering contexts.
pub(crate) struct ContextValue(String);

impl ContextValue {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    pub(crate) fn into_string(self) -> String {
        self.0
    }
}

impl From<String> for ContextValue {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Configuration values extracted from the test world for stdlib rendering.
struct RenderConfig {
    policy: Option<NetworkPolicy>,
    fetch_max_bytes: Option<u64>,
    command_max_output_bytes: Option<u64>,
    command_stream_max_bytes: Option<u64>,
}

fn extract_render_config() -> RenderConfig {
    with_world(|world| RenderConfig {
        policy: world.stdlib_policy.with_ref(|p| p.clone()),
        fetch_max_bytes: world.stdlib_fetch_max_bytes.get(),
        command_max_output_bytes: world.stdlib_command_max_output_bytes.get(),
        command_stream_max_bytes: world.stdlib_command_stream_max_bytes.get(),
    })
}

pub(crate) fn render_template_with_context(template: &TemplateContent, ctx: Value) -> Result<()> {
    let root = ensure_workspace()?;
    let mut env = Environment::new();
    let workspace = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open stdlib workspace directory")?;
    let mut config = StdlibConfig::new(workspace)?.with_workspace_root_path(root.clone())?;

    // Extract config from world before applying
    let render_cfg = extract_render_config();

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

    let state = stdlib::register_with_config(&mut env, config)?;
    state.reset_impure();

    let render = env.render_str(template.as_str(), ctx);
    with_world(|world| {
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
    });
    Ok(())
}

fn render_with_single_context(
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
    render_template_with_context(template, ctx)
}

fn render_template(template: &TemplateContent, path: &TemplatePath) -> Result<()> {
    let ctx = context!(path => path.as_path().as_str());
    render_template_with_context(template, ctx)
}

#[when("I render template {template} at stdlib path {path}")]
pub(crate) fn render_stdlib_template(template: String, path: String) -> Result<()> {
    let template = TemplateContent::new(strip_quotes(&template));
    let path = strip_quotes(&path);
    let root = ensure_workspace()?;
    let target = resolve_template_path(root.as_path(), path);
    render_template(&template, &target)
}

#[when("I render the stdlib template {template} without context")]
pub(crate) fn render_stdlib_template_without_path(template: String) -> Result<()> {
    let template = TemplateContent::new(strip_quotes(&template));
    render_template_with_context(&template, context! {})
}

#[when("I render template {template} with stdlib url")]
pub(crate) fn render_stdlib_template_with_url(template: String) -> Result<()> {
    let template = TemplateContent::new(strip_quotes(&template));
    let url = with_world(|world| {
        world
            .stdlib_url
            .get()
            .context("expected stdlib HTTP server to be initialised")
    })?;
    render_with_single_context(&template, &ContextKey::new("url"), ContextValue::new(url))
}

#[when("I render the stdlib template {template} using the stdlib command helper")]
pub(crate) fn render_stdlib_template_with_command(template: String) -> Result<()> {
    let template = TemplateContent::new(strip_quotes(&template));
    let cmd = with_world(|world| {
        world
            .stdlib_command
            .get()
            .context("expected stdlib command helper to be compiled")
    })?;
    render_with_single_context(&template, &ContextKey::new("cmd"), ContextValue::new(cmd))
}

#[when("I render the stdlib template {template} using the stdlib text")]
pub(crate) fn render_stdlib_template_with_text(template: String) -> Result<()> {
    let template = TemplateContent::new(strip_quotes(&template));
    let text = with_world(|world| {
        world
            .stdlib_text
            .get()
            .context("expected stdlib template text to be configured")
    })?;
    render_with_single_context(&template, &ContextKey::new("text"), ContextValue::new(text))
}
