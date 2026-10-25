//! Regression tests for rendering command-list entries.

use super::*;
use crate::ast::{DependencyOrder, NetsukeManifest, Recipe, Rule, StringOrList, Target, Vars};
use crate::manifest::budget::ManifestBudget;
use semver::Version;

#[test]
fn large_command_list_prepares_the_jinja_context_once() {
    reset_recipe_context_preparations();
    let mut command = StringOrList::List(
        (0..64)
            .map(|index| format!("echo {{{{ label }}}} {index} {{{{ ins }}}}"))
            .collect(),
    );
    let mut vars = Vars::new();
    vars.insert("label".into(), ManifestValue::String("rendered".into()));

    let env = Environment::new();
    let budget = ManifestBudget::default();
    let context = FieldRenderContext {
        env: &env,
        budget: &budget,
        vars: &vars,
    };
    render_recipe_string_or_list(&mut command, &context, || "render command list".into())
        .expect("shell-safe command list should render");

    assert_eq!(
        recipe_context_preparations(),
        1,
        "one recipe must prepare its Jinja context once regardless of entry count"
    );
    let rendered_entries = command.to_string_vec();
    assert_eq!(
        rendered_entries.first().map(String::as_str),
        Some("echo rendered 0 __NETSUKE_INS_PLACEHOLDER__")
    );
    assert_eq!(
        rendered_entries.last().map(String::as_str),
        Some("echo rendered 63 __NETSUKE_INS_PLACEHOLDER__")
    );
}

#[test]
fn dependency_only_marker_skips_recipe_context_preparation() {
    reset_recipe_context_preparations();
    let mut command = StringOrList::Empty;
    let env = Environment::new();
    let budget = ManifestBudget::default();
    let vars = Vars::new();
    let context = RecipeRenderContext {
        fields: FieldRenderContext {
            env: &env,
            budget: &budget,
            vars: &vars,
        },
        subject: "target",
        mode: RenderMode::Full,
    };

    render_command_recipe(&mut command, &context)
        .expect("dependency-only marker should not need recipe rendering");

    assert_eq!(
        recipe_context_preparations(),
        0,
        "dependency-only markers must skip recipe-context preparation"
    );
}

#[test]
fn target_recipe_context_reserves_ins_and_outs_placeholders() -> Result<()> {
    let env = Environment::new();
    let mut target_vars = Vars::new();
    target_vars.insert("ins".into(), ManifestValue::String("caller-ins".into()));
    target_vars.insert("outs".into(), ManifestValue::String("caller-outs".into()));
    let manifest = NetsukeManifest {
        netsuke_version: Version::parse("1.0.0")?,
        vars: Vars::new(),
        macros: Vec::new(),
        rules: Vec::new(),
        actions: Vec::new(),
        targets: vec![Target {
            name: "out".into(),
            recipe: Recipe::Command {
                command: StringOrList::List(vec!["{{ ins }}".into(), "{{ outs }}".into()]),
                isolation: crate::ast::CommandIsolation::Shared,
            },
            sources: StringOrList::Empty,
            deps: StringOrList::Empty,
            dependency_order: DependencyOrder::Parallel,
            order_only_deps: StringOrList::Empty,
            vars: target_vars,
            phony: false,
            always: false,
            console: false,
            conditional: false,
            description: None,
        }],
        defaults: Vec::new(),
    };

    let rendered = render_manifest(manifest, &env)?;
    let target = rendered
        .targets
        .first()
        .context("rendered target missing")?;
    let Recipe::Command { command, .. } = &target.recipe else {
        anyhow::bail!("expected target command recipe, got {:?}", target.recipe);
    };
    anyhow::ensure!(
        command.to_string_vec() == [crate::ir::INS_TOKEN, crate::ir::OUTS_TOKEN],
        "unexpected rendered target command list: {command:?}"
    );
    Ok(())
}

#[test]
fn command_list_renders_each_entry_with_ins_outs_placeholders() -> Result<()> {
    let env = Environment::new();
    let mut vars = Vars::new();
    vars.insert("ins".into(), ManifestValue::String("caller-ins".into()));
    vars.insert("outs".into(), ManifestValue::String("caller-outs".into()));
    let manifest = NetsukeManifest {
        netsuke_version: Version::parse("1.0.0")?,
        vars,
        macros: Vec::new(),
        rules: vec![Rule {
            name: "check".into(),
            recipe: Recipe::Command {
                command: StringOrList::List(vec![
                    "echo {{ 1 + 1 }}".into(),
                    "{{ ins }}".into(),
                    "{{ outs }}".into(),
                ]),
                isolation: crate::ast::CommandIsolation::Shared,
            },
            description: None,
        }],
        actions: Vec::new(),
        targets: Vec::new(),
        defaults: Vec::new(),
    };
    let rendered = render_manifest(manifest, &env)?;
    let rule = rendered.rules.first().context("rendered rule missing")?;
    let Recipe::Command { command, .. } = &rule.recipe else {
        anyhow::bail!("expected command recipe, got {:?}", rule.recipe);
    };
    anyhow::ensure!(
        command.to_string_vec() == ["echo 2", crate::ir::INS_TOKEN, crate::ir::OUTS_TOKEN],
        "unexpected rendered command list: {command:?}"
    );
    Ok(())
}
