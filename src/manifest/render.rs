//! Renders manifest templates using `MiniJinja` before execution.
use super::ManifestValue;
use crate::ast::{NetsukeManifest, Recipe, StringOrList, Target, Vars};
use anyhow::{Context, Result};
use minijinja::Environment;

/// Render manifest targets and rules by evaluating template expressions.
///
/// # Errors
///
/// Returns an error when a template evaluation fails or when rendered
/// values cannot be serialised back into the manifest structure.
pub fn render_manifest(
    mut manifest: NetsukeManifest,
    env: &Environment,
) -> Result<NetsukeManifest> {
    for action in &mut manifest.actions {
        render_target(action, env)?;
    }
    for target in &mut manifest.targets {
        render_target(target, env)?;
    }
    let rule_vars = manifest.vars.clone();
    for rule in &mut manifest.rules {
        render_rule(rule, env, &rule_vars)?;
    }
    Ok(manifest)
}

fn render_rule(rule: &mut crate::ast::Rule, env: &Environment, vars: &Vars) -> Result<()> {
    if let Some(desc) = &mut rule.description {
        *desc = render_str_with(env, desc, vars, || "render rule description".into())?;
    }
    render_string_or_list(&mut rule.deps, env, vars)?;
    match &mut rule.recipe {
        Recipe::Command { command } => {
            *command = render_str_with(env, command, vars, || "render rule command".into())?;
        }
        Recipe::Script { script } => {
            *script = render_str_with(env, script, vars, || "render rule script".into())?;
        }
        Recipe::Rule { rule: r } => render_string_or_list(r, env, vars)?,
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
        if let ManifestValue::String(s) = value {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Rule;
    use minijinja::Environment;
    use semver::Version;

    fn sample_manifest() -> Result<NetsukeManifest> {
        let mut target_vars = Vars::new();
        target_vars.insert("greet".into(), ManifestValue::String("hello".into()));
        target_vars.insert("subject".into(), ManifestValue::String("world".into()));
        target_vars.insert(
            "message".into(),
            ManifestValue::String("{{ greet }} {{ subject }}".into()),
        );

        let target = Target {
            name: StringOrList::String("{{ message }}!".into()),
            recipe: Recipe::Command {
                command: "{{ message }}".into(),
            },
            sources: StringOrList::List(vec!["{{ subject }}.txt".into()]),
            deps: StringOrList::Empty,
            order_only_deps: StringOrList::List(vec!["{{ subject }}.meta".into()]),
            vars: target_vars,
            phony: false,
            always: false,
        };

        let rule = Rule {
            name: "example".into(),
            recipe: Recipe::Command {
                command: "{{ 2 + 2 }}".into(),
            },
            description: Some("{{ 1 + 1 }}".into()),
            deps: StringOrList::List(vec!["{{ message }}".into()]),
        };

        let mut manifest_vars = Vars::new();
        manifest_vars.insert(
            "message".into(),
            ManifestValue::String("hello world".into()),
        );

        Ok(NetsukeManifest {
            netsuke_version: Version::parse("1.0.0")?,
            vars: manifest_vars,
            macros: Vec::new(),
            rules: vec![rule],
            actions: Vec::new(),
            targets: vec![target],
            defaults: Vec::new(),
        })
    }

    fn expect_var<'a>(vars: &'a Vars, key: &str) -> &'a str {
        vars.get(key)
            .and_then(|value| value.as_str())
            .unwrap_or_else(|| panic!("expected rendered var '{key}'"))
    }

    fn expect_string<'a>(value: &'a StringOrList, label: &str) -> &'a str {
        match value {
            StringOrList::String(item) => item,
            other => panic!("expected {label} as string, got {other:?}"),
        }
    }

    fn expect_list<'a>(value: &'a StringOrList, label: &str) -> &'a [String] {
        match value {
            StringOrList::List(items) => items,
            other => panic!("expected {label} as list, got {other:?}"),
        }
    }

    fn expect_command<'a>(recipe: &'a Recipe, label: &str) -> &'a str {
        match recipe {
            Recipe::Command { command } => command,
            other => panic!("expected {label} command recipe, got {other:?}"),
        }
    }

    fn assert_rendered_target(target: &Target) {
        assert_eq!(expect_var(&target.vars, "message"), "hello world");
        assert_eq!(expect_string(&target.name, "target name"), "hello world!");
        assert_eq!(
            expect_list(&target.sources, "target sources"),
            ["world.txt"]
        );
        assert_eq!(expect_command(&target.recipe, "target"), "hello world");
        assert_eq!(
            expect_list(&target.order_only_deps, "order-only deps"),
            ["world.meta"]
        );
    }

    fn assert_rendered_rule(rule: &Rule) {
        assert_eq!(rule.description.as_deref(), Some("2"));
        match &rule.recipe {
            Recipe::Command { command } => assert_eq!(command, "4"),
            other => panic!("expected command recipe, got {other:?}"),
        }
        match &rule.deps {
            StringOrList::List(items) => assert_eq!(items, &["hello world".to_owned()]),
            other => panic!("expected deps list, got {other:?}"),
        }
    }

    #[test]
    fn render_manifest_renders_targets_and_rules() -> Result<()> {
        let env = Environment::new();
        let manifest = sample_manifest()?;
        let rendered = render_manifest(manifest, &env)?;
        let rendered_target = rendered
            .targets
            .first()
            .context("rendered target missing")?;
        assert_rendered_target(rendered_target);
        let rendered_rule = rendered.rules.first().context("rendered rule missing")?;
        assert_rendered_rule(rendered_rule);
        Ok(())
    }
}
