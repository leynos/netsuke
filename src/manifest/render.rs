//! Renders manifest templates using `MiniJinja` before IR lowering.
//!
//! Provides [`render_manifest`], which evaluates Jinja2-style template
//! expressions in target and rule fields. Recipe rendering ensures
//! `ins`/`outs` context keys are always present, inserting
//! `__NETSUKE_INS_PLACEHOLDER__`/`__NETSUKE_OUTS_PLACEHOLDER__` when absent
//! so that [`crate::ir::cmd_interpolate`] can substitute them later.
use super::ManifestValue;
use super::jinja_macros::render_template;
use crate::ast::{NetsukeManifest, Recipe, StringOrList, Target, Vars};
use crate::ir::{INS_TOKEN, OUTS_TOKEN};
use anyhow::{Context, Result};
use minijinja::Environment;

#[cfg(test)]
use std::cell::Cell;

/// Selects which manifest fields are safe to render for the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderMode {
    /// Render every manifest field for build, generate, and manifest output.
    Full,
    /// Render discovery metadata without evaluating recipes.
    ManifestQuery,
}

/// Render manifest targets and rules by evaluating template expressions.
///
/// # Errors
///
/// Returns an error when a template evaluation fails or when rendered
/// values cannot be serialized back into the manifest structure.
pub fn render_manifest(
    mut manifest: NetsukeManifest,
    env: &Environment,
    mode: RenderMode,
) -> Result<NetsukeManifest> {
    for action in &mut manifest.actions {
        render_target(action, env, mode)?;
    }
    for target in &mut manifest.targets {
        render_target(target, env, mode)?;
    }
    let rule_vars = manifest.vars.clone();
    for rule in &mut manifest.rules {
        render_rule(rule, env, &rule_vars, mode)?;
    }
    Ok(manifest)
}

/// Render a rule's description and recipe, substituting template expressions.
///
/// # Errors
///
/// Returns an error when a description or recipe template fails to render;
/// the propagated error names the offending rule stage.
fn render_rule(
    rule: &mut crate::ast::Rule,
    env: &Environment,
    vars: &Vars,
    mode: RenderMode,
) -> Result<()> {
    render_description(&mut rule.description, env, vars, "rule")?;
    if mode == RenderMode::Full {
        render_recipe(&mut rule.recipe, env, vars, "rule")?;
    }
    Ok(())
}

/// Render a target's vars, paths, and recipe against `env`.
///
/// # Errors
///
/// Returns an error when any of the target's vars, name, sources, deps,
/// order-only deps, description, or recipe templates fail to render.
fn render_target(target: &mut Target, env: &Environment, mode: RenderMode) -> Result<()> {
    render_vars(&mut target.vars, env)?;
    render_description(&mut target.description, env, &target.vars, "target")?;
    render_string_or_list(&mut target.name, env, &target.vars)?;
    render_string_or_list(&mut target.sources, env, &target.vars)?;
    render_string_or_list(&mut target.deps, env, &target.vars)?;
    render_string_or_list(&mut target.order_only_deps, env, &target.vars)?;
    if mode == RenderMode::Full {
        render_recipe(&mut target.recipe, env, &target.vars, "target")?;
    }
    Ok(())
}

/// Render an optional target or rule description against its context.
///
/// The `subject` selects the error-context wording ("rule" or "target") so
/// that diagnostics keep naming the manifest entry being rendered.
///
/// # Errors
///
/// Returns an error when the description template fails to render; the
/// propagated error names the subject (`"rule"` or `"target"`) description.
fn render_description(
    description: &mut Option<String>,
    env: &Environment,
    vars: &Vars,
    subject: &str,
) -> Result<()> {
    if let Some(desc) = description {
        *desc = render_str_with(env, desc, vars, || format!("render {subject} description"))?;
    }
    Ok(())
}

/// Render a target or rule recipe against its context.
///
/// The `subject` selects the error-context wording ("rule" or "target") so
/// that diagnostics keep naming the manifest entry being rendered. A command
/// recipe is rendered through [`render_recipe_string_or_list`] so the `ins`/`outs`
/// placeholders stay available; a rule-reference recipe reuses
/// [`render_string_or_list`].
///
/// # Errors
///
/// Returns an error when the recipe's script or command text fails to render;
/// the propagated error names the subject (`"rule"` or `"target"`) and the
/// failing stage.
fn render_recipe(recipe: &mut Recipe, env: &Environment, vars: &Vars, subject: &str) -> Result<()> {
    match recipe {
        Recipe::Command { command } => {
            render_recipe_string_or_list(command, env, vars, || {
                format!("render {subject} command")
            })?;
        }
        Recipe::Script { script } => {
            *script = render_str_with(env, script, vars, || format!("render {subject} script"))?;
        }
        Recipe::Rule { rule } => render_string_or_list(rule, env, vars)?,
    }
    Ok(())
}

/// Render each string variable against a snapshot of the original `vars`.
///
/// # Errors
///
/// Returns an error when any string variable fails to render; the propagated
/// error names the offending variable key.
fn render_vars(vars: &mut Vars, env: &Environment) -> Result<()> {
    let snapshot = vars.clone();
    for (key, value) in vars.iter_mut() {
        if let ManifestValue::String(s) = value {
            *s = render_str_with(env, s, &snapshot, || format!("render var '{key}'"))?;
        }
    }
    Ok(())
}

/// Render every string inside a `StringOrList` against `ctx`.
///
/// # Errors
///
/// Returns an error when a scalar or list-entry template fails to render.
fn render_string_or_list(value: &mut StringOrList, env: &Environment, ctx: &Vars) -> Result<()> {
    match value {
        StringOrList::String(s) => {
            *s = render_str_with(env, s, ctx, || "render string value".into())?;
        }
        StringOrList::List(list) => {
            for item in list {
                *item = render_str_with(env, item, ctx, || "render list value".into())?;
            }
        }
        StringOrList::Empty => {}
    }
    Ok(())
}

/// Render a recipe `command` field, injecting the `ins`/`outs` placeholders
/// for every entry.
///
/// A scalar command renders as today; each entry of a list command is
/// rendered independently so `{{ ins }}`/`{{ outs }}` expand per entry during
/// IR interpolation. The `what` label is computed once and shared by every
/// entry. A scalar failure names the recipe stage alone; a list failure also
/// names the one-based position of the entry that failed to render.
///
/// # Errors
///
/// Returns an error when any command entry fails to render; the propagated
/// template error is given the named stage (`what`) context, adding the
/// entry position for list commands.
fn render_recipe_string_or_list(
    value: &mut StringOrList,
    env: &Environment,
    ctx: &Vars,
    what: impl FnOnce() -> String,
) -> Result<()> {
    let label = what();
    let recipe_ctx = recipe_render_context(ctx);
    let render_entry = |entry: &mut String, position: Option<usize>| -> Result<()> {
        *entry = render_str_with(env, entry, &recipe_ctx, || {
            position.map_or_else(|| label.clone(), |index| format!("{label} entry {index}"))
        })?;
        Ok(())
    };
    match value {
        StringOrList::String(s) => render_entry(s, None)?,
        StringOrList::List(list) => {
            for (index, item) in list.iter_mut().enumerate() {
                render_entry(item, Some(index + 1))?;
            }
        }
        StringOrList::Empty => {}
    }
    Ok(())
}

/// Clone a recipe context once, adding the delayed path placeholders.
///
/// Every list entry sees the same Jinja bindings. Keeping this preparation
/// outside the entry loop avoids cloning a target's complete `vars` map for
/// each item while retaining the scalar rendering contract.
fn recipe_render_context(ctx: &Vars) -> Vars {
    record_recipe_context_preparation();
    let mut recipe_ctx = ctx.clone();
    recipe_ctx.insert("ins".into(), ManifestValue::String(INS_TOKEN.into()));
    recipe_ctx.insert("outs".into(), ManifestValue::String(OUTS_TOKEN.into()));
    recipe_ctx
}

#[cfg(test)]
thread_local! {
    static RECIPE_CONTEXT_PREPARATIONS: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
fn record_recipe_context_preparation() {
    RECIPE_CONTEXT_PREPARATIONS.with(|count| count.set(count.get() + 1));
}

#[cfg(not(test))]
/// Count one recipe-context preparation for the recipe-context tests.
const fn record_recipe_context_preparation() {}

#[cfg(test)]
pub(super) fn reset_recipe_context_preparations() {
    RECIPE_CONTEXT_PREPARATIONS.with(|count| count.set(0));
}

#[cfg(test)]
pub(super) fn recipe_context_preparations() -> usize {
    RECIPE_CONTEXT_PREPARATIONS.with(Cell::get)
}

/// Render one template string, attaching `what` to any error context.
fn render_str_with(
    env: &Environment,
    tpl: &str,
    ctx: &impl serde::Serialize,
    what: impl FnOnce() -> String,
) -> Result<String> {
    render_template(env, tpl, ctx).with_context(what)
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "render_command_list_tests.rs"]
mod command_list_tests;
