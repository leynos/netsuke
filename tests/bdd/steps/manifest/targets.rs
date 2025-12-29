//! Target-specific step definitions for manifest parsing scenarios.
//!
//! Contains step definitions for asserting on target properties like name,
//! command, script, rule, sources, deps, and flags.

use super::helpers::{
    assert_macro_count, assert_macro_signature_eq, assert_target_always, assert_target_command_eq,
    assert_target_count, assert_target_has_dep, assert_target_has_order_only_dep,
    assert_target_has_source, assert_target_name_eq, assert_target_phony, assert_target_rule_eq,
    assert_target_script_eq, with_target,
};
use super::{get_string_from_string_or_list, with_manifest_error_context};
use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use crate::bdd::types::{
    CommandText, DepName, MacroSignature, NamesList, RuleName, ScriptText, SourcePath, TargetName,
};
use anyhow::{Context, Result, bail, ensure};
use netsuke::ast::Recipe;
use rstest_bdd_macros::{then, when};
use std::{collections::BTreeSet, convert::TryFrom};

const INDEX_KEY: &str = "index";

// ---------------------------------------------------------------------------
// Then steps - target assertions
// ---------------------------------------------------------------------------

#[expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macro generates wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[then("the first target name is {name:string}")]
fn first_target_name(world: &TestWorld, name: &str) -> Result<()> {
    let name = TargetName::new(name);
    let result = world.manifest.with_ref(|m| {
        let target = m.targets.first().context("missing target 1")?;
        let actual = get_string_from_string_or_list(&target.name, "name")?;
        assert_target_name_eq(1, &actual, &name)
    });
    with_manifest_error_context(world, result.context("manifest has not been parsed"))?
}

#[then("the target {index:usize} is phony")]
fn target_is_phony(world: &TestWorld, index: usize) -> Result<()> {
    assert_target_phony(world, index, true)
}

#[then("the target {index:usize} is always rebuilt")]
fn target_is_always(world: &TestWorld, index: usize) -> Result<()> {
    assert_target_always(world, index, true)
}

#[then("the target {index:usize} is not phony")]
fn target_not_phony(world: &TestWorld, index: usize) -> Result<()> {
    assert_target_phony(world, index, false)
}

#[then("the target {index:usize} is not always rebuilt")]
fn target_not_always(world: &TestWorld, index: usize) -> Result<()> {
    assert_target_always(world, index, false)
}

#[expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macro generates wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[then("the first target command is {command:string}")]
fn first_target_command(world: &TestWorld, command: &str) -> Result<()> {
    let command = CommandText::new(command);
    let result = world.manifest.with_ref(|m| {
        let target = m.targets.first().context("missing target 1")?;
        match &target.recipe {
            Recipe::Command { command: actual } => assert_target_command_eq(1, actual, &command),
            other => bail!("Expected command recipe, got: {other:?}"),
        }
    });
    with_manifest_error_context(world, result.context("manifest has not been parsed"))?
}

#[then("the manifest has {count:usize} targets")]
fn manifest_has_targets(world: &TestWorld, count: usize) -> Result<()> {
    assert_target_count(world, count)
}

#[then("the manifest has {count:usize} macros")]
fn manifest_has_macros(world: &TestWorld, count: usize) -> Result<()> {
    assert_macro_count(world, count)
}

#[expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macro generates wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[then("the macro {index:usize} signature is {signature:string}")]
fn macro_signature_is(world: &TestWorld, index: usize, signature: &str) -> Result<()> {
    let signature = MacroSignature::new(signature);
    ensure!(index > 0, "macros use 1-based index");
    let result = world.manifest.with_ref(|m| {
        let macro_def = m
            .macros
            .get(index - 1)
            .with_context(|| format!("missing macro {index}"))?;
        assert_macro_signature_eq(index, macro_def.signature.as_str(), &signature)
    });
    with_manifest_error_context(world, result.context("manifest has not been parsed"))?
}

#[expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macro generates wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[when("the manifest has targets named {names:string}")]
#[then("the manifest has targets named {names:string}")]
fn manifest_has_targets_named(world: &TestWorld, names: &str) -> Result<()> {
    let names = NamesList::new(names);
    let expected: BTreeSet<String> = names.to_set();
    let result = world.manifest.with_ref(|m| {
        let actual: BTreeSet<String> = m
            .targets
            .iter()
            .enumerate()
            .map(|(i, target)| {
                get_string_from_string_or_list(&target.name, "name").with_context(|| {
                    format!(
                        "failed to extract name for target at index {} (raw: {:?})",
                        i + 1,
                        target
                    )
                })
            })
            .collect::<Result<_>>()?;
        let missing: BTreeSet<_> = expected.difference(&actual).cloned().collect();
        let extra: BTreeSet<_> = actual.difference(&expected).cloned().collect();
        ensure!(
            missing.is_empty() && extra.is_empty(),
            "target names differ\nmissing: {missing:?}\nextra: {extra:?}"
        );
        Ok(())
    });
    with_manifest_error_context(world, result.context("manifest has not been parsed"))?
}

#[expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macro generates wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[then("the target {index:usize} name is {name:string}")]
fn target_name_n(world: &TestWorld, index: usize, name: &str) -> Result<()> {
    let name = TargetName::new(name);
    with_target(world, index, |target| {
        let actual = get_string_from_string_or_list(&target.name, "name")?;
        assert_target_name_eq(index, &actual, &name)
    })
}

#[expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macro generates wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[then("the target {index:usize} command is {command:string}")]
fn target_command_n(world: &TestWorld, index: usize, command: &str) -> Result<()> {
    let command = CommandText::new(command);
    with_target(world, index, |target| match &target.recipe {
        Recipe::Command { command: actual } => assert_target_command_eq(index, actual, &command),
        other => bail!("Expected command recipe, got: {other:?}"),
    })
}

#[then("the target {index:usize} index is {expected:usize}")]
fn target_index_n(world: &TestWorld, index: usize, expected: usize) -> Result<()> {
    with_target(world, index, |target| {
        let index_value = target
            .vars
            .get(INDEX_KEY)
            .with_context(|| format!("target {index} missing '{INDEX_KEY}' variable"))?
            .as_u64()
            .with_context(|| format!("target {index} index is not an integer"))?;
        let actual = usize::try_from(index_value)
            .with_context(|| format!("target {index} index does not fit into usize"))?;
        ensure!(
            actual == expected,
            "unexpected index for target {index}: expected {expected}, got {actual}"
        );
        Ok(())
    })
}

#[expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macro generates wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[then("the target {index:usize} has source {source:string}")]
fn target_has_source(world: &TestWorld, index: usize, source: &str) -> Result<()> {
    let source = SourcePath::new(source);
    with_target(world, index, |target| {
        assert_target_has_source(index, &target.sources, &source)
    })
}

#[expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macro generates wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[then("the target {index:usize} has dep {dep:string}")]
fn target_has_dep(world: &TestWorld, index: usize, dep: &str) -> Result<()> {
    let dep = DepName::new(dep);
    with_target(world, index, |target| {
        assert_target_has_dep(index, &target.deps, &dep)
    })
}

#[expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macro generates wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[then("the target {index:usize} has order-only dep {dep:string}")]
fn target_has_order_only_dep(world: &TestWorld, index: usize, dep: &str) -> Result<()> {
    let dep = DepName::new(dep);
    with_target(world, index, |target| {
        assert_target_has_order_only_dep(index, &target.order_only_deps, &dep)
    })
}

#[expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macro generates wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[then("the target {index:usize} script is {script:string}")]
fn target_script_is(world: &TestWorld, index: usize, script: &str) -> Result<()> {
    let script = ScriptText::new(script);
    with_target(world, index, |target| match &target.recipe {
        Recipe::Script { script: actual } => assert_target_script_eq(index, actual, &script),
        other => bail!("Expected script recipe, got: {other:?}"),
    })
}

#[expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macro generates wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[then("the target {index:usize} rule is {rule:string}")]
fn target_rule_is(world: &TestWorld, index: usize, rule: &str) -> Result<()> {
    let rule = RuleName::new(rule);
    with_target(world, index, |target| match &target.recipe {
        Recipe::Rule { rule: actual } => {
            let actual_str = get_string_from_string_or_list(actual, "rule")?;
            assert_target_rule_eq(index, &actual_str, &rule)
        }
        other => bail!("Expected rule recipe, got: {other:?}"),
    })
}
