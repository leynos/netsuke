//! Renders manifest templates using `MiniJinja` before IR lowering.
//!
//! Provides [`render_manifest`], which evaluates Jinja2-style template
//! expressions in target and rule fields. Recipe rendering ensures
//! `ins`/`outs` context keys are always present, inserting
//! `__NETSUKE_INS_PLACEHOLDER__`/`__NETSUKE_OUTS_PLACEHOLDER__` when absent
//! so that [`crate::ir::cmd_interpolate`] can substitute them later.
use super::jinja_macros::render_template_with_budget;
use super::{ManifestValue, budget::ManifestBudget};
use crate::ast::{NetsukeManifest, Recipe, StringOrList, Target, Vars};
use crate::ir::{INS_TOKEN, OUTS_TOKEN};
use anyhow::{Context, Result};
use minijinja::Environment;

#[cfg(test)]
use std::cell::Cell;

/// Render manifest targets and rules by evaluating template expressions.
///
/// # Errors
///
/// Returns an error when a template evaluation fails or when rendered
/// values cannot be serialized back into the manifest structure.
pub fn render_manifest(manifest: NetsukeManifest, env: &Environment) -> Result<NetsukeManifest> {
    let budget = ManifestBudget::default();
    render_manifest_with_budget(manifest, env, &budget)
}

/// Render a manifest with the caller's shared resource accounting.
pub(crate) fn render_manifest_with_budget(
    manifest: NetsukeManifest,
    env: &Environment,
    budget: &ManifestBudget,
) -> Result<NetsukeManifest> {
    render_manifest_with_mode(manifest, env, budget, RenderMode::Full)
}

/// Render discovery metadata without evaluating command or script recipes.
///
/// Rule selectors still render because manifest-query graph validation resolves
/// them. This stays crate-private so external callers retain the stable full
/// rendering API.
///
/// # Errors
///
/// Returns an error when a discovery field or rule selector template cannot
/// be evaluated or when rendered values cannot be serialized back into the
/// manifest structure.
#[cfg(test)]
pub(crate) fn render_manifest_for_manifest_query(
    manifest: NetsukeManifest,
    env: &Environment,
) -> Result<NetsukeManifest> {
    let budget = ManifestBudget::default();
    render_manifest_for_manifest_query_with_budget(manifest, env, &budget)
}

/// Render manifest-query fields with the caller's shared resource accounting.
pub(crate) fn render_manifest_for_manifest_query_with_budget(
    manifest: NetsukeManifest,
    env: &Environment,
    budget: &ManifestBudget,
) -> Result<NetsukeManifest> {
    render_manifest_with_mode(manifest, env, budget, RenderMode::ManifestQuery)
}

/// Render a manifest with the caller's field-rendering policy.
///
/// # Errors
///
/// Returns an error when a template evaluation fails or when rendered values
/// cannot be serialized back into the manifest structure.
fn render_manifest_with_mode(
    mut manifest: NetsukeManifest,
    env: &Environment,
    budget: &ManifestBudget,
    mode: RenderMode,
) -> Result<NetsukeManifest> {
    for action in &mut manifest.actions {
        render_target(action, env, budget, mode)?;
    }
    for target in &mut manifest.targets {
        render_target(target, env, budget, mode)?;
    }
    let rule_vars = manifest.vars.clone();
    for rule in &mut manifest.rules {
        render_rule(
            rule,
            &RecipeRenderContext {
                fields: FieldRenderContext {
                    env,
                    budget,
                    vars: &rule_vars,
                },
                subject: "rule",
                mode,
            },
        )?;
    }
    Ok(manifest)
}

/// Select whether a render may evaluate command and script recipe bodies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RenderMode {
    /// Render every manifest field for build, generate, and manifest output.
    Full,
    /// Render discovery metadata while preserving command and script bodies.
    ManifestQuery,
}

/// Render a rule's description and recipe, substituting template expressions.
///
/// # Errors
///
/// Returns an error when a description or recipe template fails to render;
/// the propagated error names the offending rule stage.
fn render_rule(rule: &mut crate::ast::Rule, context: &RecipeRenderContext<'_, '_>) -> Result<()> {
    render_description(&mut rule.description, &context.fields, "rule")?;
    render_recipe(&mut rule.recipe, context)?;
    Ok(())
}

/// Render a target's vars, paths, and recipe against `env`.
///
/// # Errors
///
/// Returns an error when any of the target's vars, name, sources, deps,
/// order-only deps, description, or recipe templates fail to render.
fn render_target(
    target: &mut Target,
    env: &Environment,
    budget: &ManifestBudget,
    mode: RenderMode,
) -> Result<()> {
    render_vars(&mut target.vars, env, budget)?;
    let fields = FieldRenderContext {
        env,
        budget,
        vars: &target.vars,
    };
    render_description(&mut target.description, &fields, "target")?;
    render_string_or_list(&mut target.name, env, budget, &target.vars)?;
    render_string_or_list(&mut target.sources, env, budget, &target.vars)?;
    render_string_or_list(&mut target.deps, env, budget, &target.vars)?;
    render_string_or_list(&mut target.order_only_deps, env, budget, &target.vars)?;
    render_recipe(
        &mut target.recipe,
        &RecipeRenderContext {
            fields,
            subject: "target",
            mode,
        },
    )?;
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
    context: &FieldRenderContext<'_, '_>,
    subject: &str,
) -> Result<()> {
    if let Some(desc) = description {
        *desc = render_str_with(context, desc, context.vars, || {
            format!("render {subject} description")
        })?;
    }
    Ok(())
}

/// Hold the dependencies shared by an entry's ordinary rendered fields.
struct FieldRenderContext<'env, 'a> {
    /// Supplies templates with registered helpers and globals.
    env: &'a Environment<'env>,
    /// Shares evaluation limits across every target field.
    budget: &'a ManifestBudget,
    /// Supplies entry-local variables to rendered fields.
    vars: &'a Vars,
}

/// Retain recipe-specific rendering policy beside shared field state.
struct RecipeRenderContext<'env, 'a> {
    /// Supplies the common environment, budget, and variables.
    fields: FieldRenderContext<'env, 'a>,
    /// Names the manifest entry in rendering diagnostics.
    subject: &'a str,
    /// Selects whether command and script bodies are safe to evaluate.
    mode: RenderMode,
}

/// Render a recipe according to its field-rendering policy.
///
/// # Errors
///
/// Returns an error when a rendered rule selector or full-mode command or
/// script cannot be evaluated.
fn render_recipe(recipe: &mut Recipe, context: &RecipeRenderContext<'_, '_>) -> Result<()> {
    match recipe {
        Recipe::Command { command } => render_command_recipe(command, context),
        Recipe::Script { script } => render_script_recipe(script, context),
        Recipe::Rule { rule } => render_string_or_list(
            rule,
            context.fields.env,
            context.fields.budget,
            context.fields.vars,
        ),
    }
}

/// Render a command recipe only when the caller permits recipe bodies.
///
/// # Errors
///
/// Returns an error when a full-mode command template cannot be evaluated.
fn render_command_recipe(
    command: &mut StringOrList,
    context: &RecipeRenderContext<'_, '_>,
) -> Result<()> {
    if context.mode == RenderMode::ManifestQuery || command.is_empty_marker() {
        return Ok(());
    }
    render_recipe_string_or_list(command, &context.fields, || {
        format!("render {} command", context.subject)
    })
}

/// Render a script recipe only when the caller permits recipe bodies.
///
/// # Errors
///
/// Returns an error when a full-mode script template cannot be evaluated.
fn render_script_recipe(script: &mut String, context: &RecipeRenderContext<'_, '_>) -> Result<()> {
    if context.mode == RenderMode::ManifestQuery {
        return Ok(());
    }
    let recipe_context = recipe_render_context(context.fields.vars);
    *script = render_str_with(&context.fields, script, &recipe_context, || {
        format!("render {} script", context.subject)
    })?;
    Ok(())
}

/// Render each string variable against a snapshot of the original `vars`.
///
/// # Errors
///
/// Returns an error when any string variable fails to render; the propagated
/// error names the offending variable key.
fn render_vars(vars: &mut Vars, env: &Environment, budget: &ManifestBudget) -> Result<()> {
    let snapshot = vars.clone();
    let context = FieldRenderContext {
        env,
        budget,
        vars: &snapshot,
    };
    for (key, value) in vars.iter_mut() {
        if let ManifestValue::String(s) = value {
            *s = render_str_with(&context, s, &snapshot, || format!("render var '{key}'"))?;
        }
    }
    Ok(())
}

/// Render every string inside a `StringOrList` against `ctx`.
///
/// # Errors
///
/// Returns an error when a scalar or list-entry template fails to render.
fn render_string_or_list(
    value: &mut StringOrList,
    env: &Environment,
    budget: &ManifestBudget,
    ctx: &Vars,
) -> Result<()> {
    let context = FieldRenderContext {
        env,
        budget,
        vars: ctx,
    };
    match value {
        StringOrList::String(s) => {
            *s = render_str_with(&context, s, ctx, || "render string value".into())?;
        }
        StringOrList::List(list) => {
            for item in list {
                *item = render_str_with(&context, item, ctx, || "render list value".into())?;
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
    context: &FieldRenderContext<'_, '_>,
    what: impl FnOnce() -> String,
) -> Result<()> {
    let label = what();
    let recipe_ctx = recipe_render_context(context.vars);
    let render_entry = |entry: &mut String, position: Option<usize>| -> Result<()> {
        *entry = render_str_with(context, entry, &recipe_ctx, || {
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
/// Count one recipe-context preparation for the recipe-context tests.
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
    context: &FieldRenderContext<'_, '_>,
    tpl: &str,
    ctx: &impl serde::Serialize,
    what: impl FnOnce() -> String,
) -> Result<String> {
    render_template_with_budget(context.env, tpl, ctx, context.budget).with_context(what)
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "render_command_list_tests.rs"]
mod command_list_tests;

#[cfg(test)]
#[path = "render_script_tests.rs"]
mod script_tests;
