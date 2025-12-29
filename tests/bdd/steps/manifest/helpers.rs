//! Helper functions and assertion utilities for target step definitions.
//!
//! Provides typed assertion wrappers, `StringOrList` validators, and target
//! accessor functions. These utilities are shared across target-specific BDD
//! steps to reduce duplication and improve error context.

use super::with_manifest_error_context;
use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use crate::bdd::types::{
    CommandText, DepName, MacroSignature, RuleName, ScriptText, SourcePath, TargetName,
};
use anyhow::{Context, Result, bail, ensure};
use netsuke::ast::StringOrList;

// ---------------------------------------------------------------------------
// Domain-specific typed assertion functions
// ---------------------------------------------------------------------------

/// Location information for field assertions.
#[derive(Copy, Clone)]
pub(super) struct FieldLocation {
    context: &'static str,
    index: Option<usize>,
    field: &'static str,
}

impl FieldLocation {
    /// Create a location with an index.
    pub(super) const fn with_index(
        context: &'static str,
        index: usize,
        field: &'static str,
    ) -> Self {
        Self {
            context,
            index: Some(index),
            field,
        }
    }

    /// Format the location prefix for error messages.
    fn format_prefix(self) -> String {
        self.index.map_or_else(
            || self.context.to_owned(),
            |idx| format!("{} {idx}", self.context),
        )
    }
}

/// Generic string equality assertion with contextual error messages.
pub(super) fn assert_string_eq<T>(location: FieldLocation, actual: &str, expected: &T) -> Result<()>
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
        pub(super) fn $name(index: usize, actual: &str, expected: &$type) -> Result<()> {
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

pub(super) fn assert_target_has_source(
    target_index: usize,
    sources: &StringOrList,
    expected: &SourcePath,
) -> Result<()> {
    assert_list_contains(sources, expected.as_str())
        .with_context(|| format!("target {target_index} missing source '{expected}'"))
}

pub(super) fn assert_target_has_dep(
    target_index: usize,
    deps: &StringOrList,
    expected: &DepName,
) -> Result<()> {
    assert_list_contains(deps, expected.as_str())
        .with_context(|| format!("target {target_index} missing dep '{expected}'"))
}

pub(super) fn assert_target_has_order_only_dep(
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
pub(super) fn with_target<T, F>(world: &TestWorld, index: usize, f: F) -> Result<T>
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
pub(super) fn assert_target_phony(world: &TestWorld, index: usize, expected: bool) -> Result<()> {
    with_target(world, index, |target| {
        ensure!(
            target.phony == expected,
            "target {index} phony should be {expected}"
        );
        Ok(())
    })
}

/// Validate the always flag on a target.
pub(super) fn assert_target_always(world: &TestWorld, index: usize, expected: bool) -> Result<()> {
    with_target(world, index, |target| {
        ensure!(
            target.always == expected,
            "target {index} always should be {expected}"
        );
        Ok(())
    })
}

/// Validate the number of targets in the manifest.
pub(super) fn assert_target_count(world: &TestWorld, expected: usize) -> Result<()> {
    let count = world
        .manifest
        .with_ref(|m| m.targets.len())
        .context("manifest has not been parsed");
    let actual = with_manifest_error_context(world, count)?;
    ensure!(
        actual == expected,
        "expected manifest to have {expected} targets, got {actual}"
    );
    Ok(())
}

/// Validate the number of macros in the manifest.
pub(super) fn assert_macro_count(world: &TestWorld, expected: usize) -> Result<()> {
    let count = world
        .manifest
        .with_ref(|m| m.macros.len())
        .context("manifest has not been parsed");
    let actual = with_manifest_error_context(world, count)?;
    ensure!(
        actual == expected,
        "expected manifest to have {expected} macros, got {actual}"
    );
    Ok(())
}
