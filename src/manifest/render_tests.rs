//! Unit tests for manifest template rendering.

use super::{ManifestValue, RenderMode, render_manifest};
use crate::ast::{DependencyOrder, NetsukeManifest, Recipe, Rule, StringOrList, Target, Vars};
use anyhow::{Context, Result};
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
        dependency_order: DependencyOrder::Parallel,
        order_only_deps: StringOrList::List(vec!["{{ subject }}.meta".into()]),
        vars: target_vars,
        phony: false,
        always: false,
        conditional: false,
        description: Some("{{ message }}".into()),
    };

    let rule = Rule {
        name: "example".into(),
        recipe: Recipe::Command {
            command: "{{ 2 + 2 }}".into(),
        },
        description: Some("{{ 1 + 1 }}".into()),
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

#[expect(clippy::panic, reason = "panic for clearer test failures")]
fn expect_var(vars: &Vars, key: impl AsRef<str>) -> &str {
    let key_ref = key.as_ref();
    let Some(value) = vars.get(key_ref).and_then(|value| value.as_str()) else {
        panic!("expected rendered var '{key_ref}'");
    };
    value
}

#[expect(clippy::panic, reason = "panic for clearer test failures")]
fn expect_string(value: &StringOrList, label: impl std::fmt::Display) -> &str {
    match value {
        StringOrList::String(item) => item,
        other => panic!("expected {label} as string, got {other:?}"),
    }
}

#[expect(clippy::panic, reason = "panic for clearer test failures")]
fn expect_list(value: &StringOrList, label: impl std::fmt::Display) -> &[String] {
    match value {
        StringOrList::List(items) => items,
        other => panic!("expected {label} as list, got {other:?}"),
    }
}

#[expect(clippy::panic, reason = "panic for clearer test failures")]
fn expect_command(recipe: &Recipe, label: impl std::fmt::Display) -> &str {
    match recipe {
        Recipe::Command { command } => match command {
            StringOrList::String(item) => item,
            other => panic!("expected {label} command as a scalar, got {other:?}"),
        },
        other => panic!("expected {label} command recipe, got {other:?}"),
    }
}

fn expect_script(recipe: &Recipe, label: impl std::fmt::Display) -> Result<&str> {
    match recipe {
        Recipe::Script { script } => Ok(script),
        other => anyhow::bail!("expected {label} script recipe, got {other:?}"),
    }
}

fn expect_rule_ref(recipe: &Recipe, label: impl std::fmt::Display) -> Result<&StringOrList> {
    match recipe {
        Recipe::Rule { rule } => Ok(rule),
        other => anyhow::bail!("expected {label} rule-reference recipe, got {other:?}"),
    }
}

fn assert_rendered_target(target: &Target) {
    assert_eq!(target.dependency_order, DependencyOrder::Parallel);
    assert_eq!(expect_var(&target.vars, "message"), "hello world");
    assert_eq!(
        target.description.as_deref(),
        Some("hello world"),
        "target description should be rendered through the target vars"
    );
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
        Recipe::Command { command } => assert_eq!(command.as_single(), Some("4")),
        other => panic!("expected command recipe, got {other:?}"),
    }
}

#[test]
fn render_manifest_renders_targets_and_rules() -> Result<()> {
    let env = Environment::new();
    let manifest = sample_manifest()?;
    let rendered = render_manifest(manifest, &env, RenderMode::Full)?;
    let rendered_target = rendered
        .targets
        .first()
        .context("rendered target missing")?;
    assert_rendered_target(rendered_target);
    let rendered_rule = rendered.rules.first().context("rendered rule missing")?;
    assert_rendered_rule(rendered_rule);
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
            },
            description: None,
        }],
        actions: Vec::new(),
        targets: Vec::new(),
        defaults: Vec::new(),
    };
    let rendered = render_manifest(manifest, &env, RenderMode::Full)?;
    let rule = rendered.rules.first().context("rendered rule missing")?;
    let Recipe::Command { command } = &rule.recipe else {
        anyhow::bail!("expected command recipe, got {:?}", rule.recipe);
    };
    anyhow::ensure!(
        command.to_string_vec() == ["echo 2", crate::ir::INS_TOKEN, crate::ir::OUTS_TOKEN],
        "unexpected rendered command list: {command:?}"
    );
    Ok(())
}

#[test]
fn command_list_render_failure_names_the_failing_entry() -> Result<()> {
    let env = Environment::new();
    let manifest = NetsukeManifest {
        netsuke_version: Version::parse("1.0.0")?,
        vars: Vars::new(),
        macros: Vec::new(),
        rules: vec![Rule {
            name: "check".into(),
            recipe: Recipe::Command {
                command: StringOrList::List(vec!["echo ok".into(), "echo {{ 1 + }}".into()]),
            },
            description: None,
        }],
        actions: Vec::new(),
        targets: Vec::new(),
        defaults: Vec::new(),
    };
    let error = render_manifest(manifest, &env, RenderMode::Full)
        .err()
        .context("expected the malformed entry to fail rendering")?;
    let report = format!("{error:#}");
    anyhow::ensure!(
        report.contains("render rule command entry 2"),
        "error should name the failing list position, got: {report}"
    );
    Ok(())
}

fn assert_rendered_script_and_rule_recipes(rendered: &NetsukeManifest) -> Result<()> {
    let rendered_target = rendered
        .targets
        .first()
        .context("rendered script target missing")?;
    anyhow::ensure!(
        expect_script(&rendered_target.recipe, "rendered script target")? == "echo world",
        "expected rendered script target recipe to equal 'echo world'"
    );
    let rendered_rule = rendered
        .rules
        .first()
        .context("rendered rule-reference rule missing")?;
    anyhow::ensure!(
        expect_list(
            expect_rule_ref(&rendered_rule.recipe, "rendered rule reference")?,
            "rule reference names",
        ) == ["base"],
        "expected rendered rule-reference names to equal ['base']"
    );
    Ok(())
}

#[test]
fn render_manifest_renders_script_and_rule_ref_recipes() -> Result<()> {
    let mut target_vars = Vars::new();
    target_vars.insert("subject".into(), ManifestValue::String("world".into()));
    let target = Target {
        name: StringOrList::String("scripted".into()),
        recipe: Recipe::Script {
            script: "echo {{ subject }}".into(),
        },
        sources: StringOrList::Empty,
        deps: StringOrList::Empty,
        dependency_order: DependencyOrder::Parallel,
        order_only_deps: StringOrList::Empty,
        vars: target_vars,
        phony: false,
        always: false,
        conditional: false,
        description: None,
    };
    let rule = Rule {
        name: "delegating".into(),
        recipe: Recipe::Rule {
            rule: StringOrList::List(vec!["{{ rule_name }}".into()]),
        },
        description: None,
    };
    let mut manifest_vars = Vars::new();
    manifest_vars.insert("rule_name".into(), ManifestValue::String("base".into()));

    let manifest = NetsukeManifest {
        netsuke_version: Version::parse("1.0.0")?,
        vars: manifest_vars,
        macros: Vec::new(),
        rules: vec![rule],
        actions: Vec::new(),
        targets: vec![target],
        defaults: Vec::new(),
    };

    let rendered = render_manifest(manifest, &minijinja::Environment::new(), RenderMode::Full)?;
    assert_rendered_script_and_rule_recipes(&rendered)?;
    Ok(())
}

fn manifest_with_build_only_recipe_helper() -> Result<NetsukeManifest> {
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
            vars: Vars::new(),
            phony: true,
            always: false,
            conditional: false,
            description: Some("Run tests".into()),
        }],
        targets: Vec::new(),
        defaults: Vec::new(),
    })
}

#[test]
fn manifest_query_keeps_build_only_recipe_helpers_unrendered() -> Result<()> {
    let manifest = manifest_with_build_only_recipe_helper()?;
    let rendered = render_manifest(manifest, &Environment::new(), RenderMode::ManifestQuery)?;
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

#[test]
fn full_render_evaluates_build_only_recipe_helpers() -> Result<()> {
    let mut env = Environment::new();
    env.add_function("command_available", |_command: String| true);
    let manifest = manifest_with_build_only_recipe_helper()?;
    let rendered = render_manifest(manifest, &env, RenderMode::Full)?;
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
