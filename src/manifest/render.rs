use crate::ast::{NetsukeManifest, Recipe, StringOrList, Target, Vars};
use anyhow::{Context, Result};
use minijinja::{Environment, context};
use serde_yml::Value as YamlValue;

pub(crate) fn render_manifest(
    mut manifest: NetsukeManifest,
    env: &Environment,
) -> Result<NetsukeManifest> {
    for action in &mut manifest.actions {
        render_target(action, env)?;
    }
    for target in &mut manifest.targets {
        render_target(target, env)?;
    }
    for rule in &mut manifest.rules {
        render_rule(rule, env)?;
    }
    Ok(manifest)
}

fn render_rule(rule: &mut crate::ast::Rule, env: &Environment) -> Result<()> {
    if let Some(desc) = &mut rule.description {
        *desc = render_str_with(env, desc, &context! {}, || "render rule description".into())?;
    }
    render_string_or_list(&mut rule.deps, env, &Vars::new())?;
    match &mut rule.recipe {
        Recipe::Command { command } => {
            *command =
                render_str_with(env, command, &context! {}, || "render rule command".into())?;
        }
        Recipe::Script { script } => {
            *script = render_str_with(env, script, &context! {}, || "render rule script".into())?;
        }
        Recipe::Rule { rule: r } => render_string_or_list(r, env, &Vars::new())?,
    }
    Ok(())
}

fn render_target(target: &mut Target, env: &Environment) -> Result<()> {
    render_vars(&mut target.vars, env)?;
    render_string_or_list(&mut target.name, env, &target.vars)?;
    render_string_or_list(&mut target.sources, env, &target.vars)?;
    render_string_or_list(&mut target.deps, env, &target.vars)?;
    render_string_or_list(&mut target.order_only_deps, env, &target.vars)?;
    match &mut target.recipe {
        Recipe::Command { command } => {
            *command = render_str_with(env, command, &target.vars, || {
                "render target command".into()
            })?;
        }
        Recipe::Script { script } => {
            *script = render_str_with(env, script, &target.vars, || "render target script".into())?;
        }
        Recipe::Rule { rule } => render_string_or_list(rule, env, &target.vars)?,
    }
    Ok(())
}

fn render_vars(vars: &mut Vars, env: &Environment) -> Result<()> {
    let snapshot = vars.clone();
    for (key, value) in vars.iter_mut() {
        if let YamlValue::String(s) = value {
            *s = render_str_with(env, s, &snapshot, || format!("render var '{key}'"))?;
        }
    }
    Ok(())
}

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

fn render_str_with(
    env: &Environment,
    tpl: &str,
    ctx: &impl serde::Serialize,
    what: impl FnOnce() -> String,
) -> Result<String> {
    env.render_str(tpl, ctx).with_context(what)
}
