//! Regression tests for script recipe rendering.

use super::*;
use crate::ast::{DependencyOrder, Target};
use anyhow::{Context, Result, ensure};
use semver::Version;

/// Verify script recipes reserve delayed path placeholders over target variables.
#[test]
fn script_recipe_context_reserves_ins_and_outs_placeholders() -> Result<()> {
    let mut target_vars = Vars::new();
    target_vars.insert("ins".into(), ManifestValue::String("caller-ins".into()));
    target_vars.insert("outs".into(), ManifestValue::String("caller-outs".into()));
    target_vars.insert("subject".into(), ManifestValue::String("rendered".into()));
    let manifest = NetsukeManifest {
        netsuke_version: Version::parse("1.0.0")?,
        vars: Vars::new(),
        macros: Vec::new(),
        rules: Vec::new(),
        actions: Vec::new(),
        targets: vec![Target {
            name: "out".into(),
            recipe: Recipe::Script {
                script: "echo {{ ins }} {{ outs }} {{ subject }}".into(),
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
        }],
        defaults: Vec::new(),
    };

    let rendered = render_manifest(manifest, &Environment::new())?;
    let target = rendered
        .targets
        .first()
        .context("rendered script target missing")?;
    let Recipe::Script { script } = &target.recipe else {
        anyhow::bail!("expected script recipe, got {:?}", target.recipe);
    };
    ensure!(
        script == &format!("echo {INS_TOKEN} {OUTS_TOKEN} rendered"),
        "script must reserve ins and outs while rendering ordinary variables: {script:?}"
    );
    Ok(())
}
