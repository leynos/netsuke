//! Target-specific step definitions for manifest parsing scenarios.
//!
//! Contains step definitions for asserting on target properties like name,
//! command, script, rule, sources, deps, and flags.

use super::{get_string_from_string_or_list, with_manifest_error_context};
use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use crate::bdd::types::{
    CommandText, DepName, MacroSignature, NamesList, RuleName, ScriptText, SourcePath, TargetName,
};
use anyhow::{Context, Result, bail, ensure};
use netsuke::ast::{Recipe, StringOrList};
use rstest_bdd_macros::{then, when};
use std::{collections::BTreeSet, convert::TryFrom};

const INDEX_KEY: &str = "index";

// ---------------------------------------------------------------------------
// Domain-specific typed assertion functions
// ---------------------------------------------------------------------------

/// Location information for field assertions.
struct FieldLocation {
    context: &'static str,
    index: Option<usize>,
    field: &'static str,
}

impl FieldLocation {
    /// Create a location with an index.
    const fn with_index(context: &'static str, index: usize, field: &'static str) -> Self {
        Self {
            context,
            index: Some(index),
            field,
        }
    }

    /// Format the location prefix for error messages.
    fn format_prefix(&self) -> String {
        match self.index {
            Some(idx) => format!("{} {idx}", self.context),
            None => self.context.to_string(),
        }
    }
}

/// Generic string equality assertion with contextual error messages.
fn assert_string_eq<T>(location: FieldLocation, actual: &str, expected: &T) -> Result<()>
where
    T: AsRef<str> + std::fmt::Display,
{
    let prefix = location.format_prefix();
    ensure!(
        actual == expected.as_ref(),
        "expected {prefix} {} '{expected}', got '{actual}'",
        location.field
    );
    Ok(())
}

/// Generates typed assertion wrapper functions around `assert_string_eq`.
///
/// This macro reduces boilerplate for assertion functions that compare actual
/// string values against expected typed values.
macro_rules! define_assertion {
    (indexed: $name:ident, $context:literal, $field:literal, $type:ty) => {
        fn $name(index: usize, actual: &str, expected: &$type) -> Result<()> {
            assert_string_eq(
                FieldLocation::with_index($context, index, $field),
                actual,
                expected,
            )
        }
    };
}

define_assertion!(indexed: assert_target_name_eq, "target", "name", TargetName);
define_assertion!(indexed: assert_target_command_eq, "target", "command", CommandText);
define_assertion!(indexed: assert_target_script_eq, "target", "script", ScriptText);
define_assertion!(indexed: assert_target_rule_eq, "target", "rule", RuleName);
define_assertion!(indexed: assert_macro_signature_eq, "macro", "signature", MacroSignature);

fn assert_target_has_source(
    target_index: usize,
    sources: &StringOrList,
    expected: &SourcePath,
) -> Result<()> {
    assert_list_contains(sources, expected.as_str())
        .with_context(|| format!("target {target_index} missing source '{expected}'"))
}

fn assert_target_has_dep(
    target_index: usize,
    deps: &StringOrList,
    expected: &DepName,
) -> Result<()> {
    assert_list_contains(deps, expected.as_str())
        .with_context(|| format!("target {target_index} missing dep '{expected}'"))
}

fn assert_target_has_order_only_dep(
    target_index: usize,
    deps: &StringOrList,
    expected: &DepName,
) -> Result<()> {
    assert_list_contains(deps, expected.as_str())
        .with_context(|| format!("target {target_index} missing order-only dep '{expected}'"))
}

fn assert_list_contains(value: &StringOrList, expected: &str) -> Result<()> {
    match value {
        StringOrList::List(list) => ensure!(
            list.iter().any(|entry| entry == expected),
            "missing {expected}"
        ),
        StringOrList::String(s) => ensure!(s == expected, "expected '{expected}', got '{s}'"),
        StringOrList::Empty => bail!("value is empty"),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Generic helper functions for target access
// ---------------------------------------------------------------------------

/// Access a target by 1-based index and apply a closure to it.
fn with_target<T, F>(world: &TestWorld, index: usize, f: F) -> Result<T>
where
    F: FnOnce(&netsuke::ast::Target) -> Result<T>,
{
    ensure!(index > 0, "target index is 1-based");
    let result = world.manifest.with_ref(|m| {
        let target = m
            .targets
            .get(index - 1)
            .with_context(|| format!("missing target {index}"))?;
        f(target)
    });
    with_manifest_error_context(world, result.context("manifest has not been parsed"))?
}

/// Validate the phony flag on a target.
fn assert_target_phony(world: &TestWorld, index: usize, expected: bool) -> Result<()> {
    with_target(world, index, |target| {
        ensure!(
            target.phony == expected,
            "target {index} phony should be {expected}"
        );
        Ok(())
    })
}

/// Validate the always flag on a target.
fn assert_target_always(world: &TestWorld, index: usize, expected: bool) -> Result<()> {
    with_target(world, index, |target| {
        ensure!(
            target.always == expected,
            "target {index} always should be {expected}"
        );
        Ok(())
    })
}

/// Validate the number of targets in the manifest.
fn assert_target_count(world: &TestWorld, expected: usize) -> Result<()> {
    let actual = world
        .manifest
        .with_ref(|m| m.targets.len())
        .context("manifest has not been parsed");
    let actual = with_manifest_error_context(world, actual)?;
    ensure!(
        actual == expected,
        "expected manifest to have {expected} targets, got {actual}"
    );
    Ok(())
}

/// Validate the number of macros in the manifest.
fn assert_macro_count(world: &TestWorld, expected: usize) -> Result<()> {
    let actual = world
        .manifest
        .with_ref(|m| m.macros.len())
        .context("manifest has not been parsed");
    let actual = with_manifest_error_context(world, actual)?;
    ensure!(
        actual == expected,
        "expected manifest to have {expected} macros, got {actual}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Then steps - target assertions
// ---------------------------------------------------------------------------

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

#[then("the target {index:usize} name is {name:string}")]
fn target_name_n(world: &TestWorld, index: usize, name: &str) -> Result<()> {
    let name = TargetName::new(name);
    with_target(world, index, |target| {
        let actual = get_string_from_string_or_list(&target.name, "name")?;
        assert_target_name_eq(index, &actual, &name)
    })
}

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

#[then("the target {index:usize} has source {source:string}")]
fn target_has_source(world: &TestWorld, index: usize, source: &str) -> Result<()> {
    let source = SourcePath::new(source);
    with_target(world, index, |target| {
        assert_target_has_source(index, &target.sources, &source)
    })
}

#[then("the target {index:usize} has dep {dep:string}")]
fn target_has_dep(world: &TestWorld, index: usize, dep: &str) -> Result<()> {
    let dep = DepName::new(dep);
    with_target(world, index, |target| {
        assert_target_has_dep(index, &target.deps, &dep)
    })
}

#[then("the target {index:usize} has order-only dep {dep:string}")]
fn target_has_order_only_dep(world: &TestWorld, index: usize, dep: &str) -> Result<()> {
    let dep = DepName::new(dep);
    with_target(world, index, |target| {
        assert_target_has_order_only_dep(index, &target.order_only_deps, &dep)
    })
}

#[then("the target {index:usize} script is {script:string}")]
fn target_script_is(world: &TestWorld, index: usize, script: &str) -> Result<()> {
    let script = ScriptText::new(script);
    with_target(world, index, |target| match &target.recipe {
        Recipe::Script { script: actual } => assert_target_script_eq(index, actual, &script),
        other => bail!("Expected script recipe, got: {other:?}"),
    })
}

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
