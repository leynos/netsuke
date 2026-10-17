//! Verify query rendering preserves recipe bodies and resolves structure.

use super::super::{ManifestValue, render_manifest, render_manifest_for_manifest_query};
use super::{expect_command, expect_rule_ref, expect_script, expect_string};
use crate::ast::{DependencyOrder, NetsukeManifest, Recipe, Rule, StringOrList, Target, Vars};
use anyhow::{Context, Result};
use minijinja::Environment;
use semver::Version;

/// Create a manifest fixture with an action recipe using the build-only `command_available` helper.
fn manifest_with_build_only_recipe_helper() -> Result<NetsukeManifest> {
    let mut action_vars = Vars::new();
    action_vars.insert(
        "description".into(),
        ManifestValue::String("Run tests".into()),
    );
    Ok(NetsukeManifest {
        netsuke_version: Version::parse("1.0.0")?,
        vars: Vars::new(),
        macros: Vec::new(),
        rules: Vec::new(),
        actions: vec![Target {
            name: "test".into(),
            recipe: Recipe::Command {
                command: concat!(
                    "cargo {% if command_available(\"cargo-nextest\") %}",
                    "nextest run{% else %}test{% endif %} --all-targets"
                )
                .into(),
            },
            sources: StringOrList::Empty,
            deps: StringOrList::Empty,
            dependency_order: DependencyOrder::Parallel,
            order_only_deps: StringOrList::Empty,
            vars: action_vars,
            phony: true,
            always: false,
            conditional: false,
            description: Some("{{ description }}".into()),
        }],
        targets: Vec::new(),
        defaults: Vec::new(),
    })
}

/// Verify that query rendering preserves build-only command helpers.
#[test]
fn manifest_query_keeps_build_only_recipe_helpers_unrendered() -> Result<()> {
    let manifest = manifest_with_build_only_recipe_helper()?;
    let rendered = render_manifest_for_manifest_query(manifest, &Environment::new())?;
    let action = rendered
        .actions
        .first()
        .context("rendered action missing")?;

    anyhow::ensure!(
        expect_command(&action.recipe, "query action").contains("command_available"),
        "manifest query should leave build-only recipe helpers unrendered"
    );
    anyhow::ensure!(
        action.description.as_deref() == Some("Run tests"),
        "manifest query should still render discovery descriptions"
    );
    Ok(())
}

/// Verify that query rendering leaves rule command and script bodies raw.
#[test]
fn manifest_query_keeps_rule_recipe_bodies_unrendered() -> Result<()> {
    let manifest = NetsukeManifest {
        netsuke_version: Version::parse("1.0.0")?,
        vars: Vars::new(),
        macros: Vec::new(),
        rules: vec![
            Rule {
                name: "command-rule".into(),
                recipe: Recipe::Command {
                    command: "{{ command_available('compiler') }} {{ ins }}".into(),
                },
                description: Some("Command rule".into()),
            },
            Rule {
                name: "script-rule".into(),
                recipe: Recipe::Script {
                    script: "{{ shell('echo ignored') }} {{ outs }}".into(),
                },
                description: None,
            },
        ],
        actions: Vec::new(),
        targets: Vec::new(),
        defaults: Vec::new(),
    };

    let rendered = render_manifest_for_manifest_query(manifest, &Environment::new())?;
    let command = rendered.rules.first().context("command rule missing")?;
    let script = rendered.rules.get(1).context("script rule missing")?;
    anyhow::ensure!(
        expect_command(&command.recipe, "query rule")
            == "{{ command_available('compiler') }} {{ ins }}",
        "query mode should retain a rule command body verbatim"
    );
    anyhow::ensure!(
        expect_script(&script.recipe, "query rule")? == "{{ shell('echo ignored') }} {{ outs }}",
        "query mode should retain a rule script body verbatim"
    );
    Ok(())
}

/// Verify that full rendering evaluates build-only command helpers.
#[test]
fn full_render_evaluates_build_only_recipe_helpers() -> Result<()> {
    let mut env = Environment::new();
    env.add_function("command_available", |_command: String| true);
    let manifest = manifest_with_build_only_recipe_helper()?;
    let rendered = render_manifest(manifest, &env)?;
    let action = rendered
        .actions
        .first()
        .context("rendered action missing")?;

    anyhow::ensure!(
        expect_command(&action.recipe, "full action") == "cargo nextest run --all-targets",
        "full rendering should evaluate build-only recipe helpers"
    );
    Ok(())
}

/// Render the build-only-helper fixture with a replacement recipe in query mode.
fn render_fixture_recipe_in_manifest_query(recipe: Recipe) -> Result<Target> {
    let mut manifest = manifest_with_build_only_recipe_helper()?;
    let action = manifest
        .actions
        .first_mut()
        .context("fixture action missing")?;
    action.recipe = recipe;
    render_manifest_for_manifest_query(manifest, &Environment::new())?
        .actions
        .into_iter()
        .next()
        .context("rendered action missing")
}

/// Verify that query rendering preserves build-only script helpers.
#[test]
fn manifest_query_keeps_build_only_script_helpers_unrendered() -> Result<()> {
    let script = "echo {{ command_available(\"cargo-nextest\") }}";
    let action = render_fixture_recipe_in_manifest_query(Recipe::Script {
        script: script.into(),
    })?;
    anyhow::ensure!(
        expect_script(&action.recipe, "query action")? == script,
        "manifest query should leave build-only script helpers unrendered"
    );
    Ok(())
}

/// Verify that query rendering resolves structural rule selectors.
#[test]
fn manifest_query_renders_rule_recipe_selectors() -> Result<()> {
    let action = render_fixture_recipe_in_manifest_query(Recipe::Rule {
        rule: "{{ description }}".into(),
    })?;
    anyhow::ensure!(
        expect_string(
            expect_rule_ref(&action.recipe, "query action rule reference")?,
            "query action rule reference",
        ) == "Run tests",
        "manifest query should render rule recipe selectors"
    );
    Ok(())
}
