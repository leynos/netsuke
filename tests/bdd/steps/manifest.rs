//! Step definitions for manifest parsing scenarios.

use crate::bdd::fixtures::{RefCellOptionExt, with_world};
use crate::bdd::types::{
    CommandText, DepName, EnvVarKey, EnvVarValue, ErrorPattern, MacroSignature, ManifestPath,
    NamesList, RuleName, ScriptText, SourcePath, TargetName, VersionString,
};
use anyhow::{Context, Result, bail, ensure};
use netsuke::{
    ast::{Recipe, StringOrList},
    manifest,
};
use rstest_bdd_macros::{given, then, when};
use std::{collections::BTreeSet, convert::TryFrom, ffi::OsStr};
use test_support::display_error_chain;
use test_support::env::{remove_var, set_var};

const INDEX_KEY: &str = "index";

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Enhance an error with manifest parse error context if available.
///
/// When manifest parsing fails, the error is stored in `manifest_error` but
/// not propagated. This helper retrieves any stored error and includes it
/// in the error context, making diagnosis easier.
fn with_manifest_error_context<T>(result: Result<T>) -> Result<T> {
    if result.is_ok() {
        return result;
    }
    with_world(|world| {
        if let Some(parse_err) = world.manifest_error.get() {
            result.with_context(|| format!("manifest parse error: {parse_err}"))
        } else {
            result
        }
    })
}

fn get_string_from_string_or_list(value: &StringOrList, field_name: &str) -> Result<String> {
    match value {
        StringOrList::String(s) => Ok(s.clone()),
        StringOrList::List(list) => {
            ensure!(
                list.len() == 1,
                "Expected String or single-item List for {field_name}, got list of length {}",
                list.len()
            );
            list.first()
                .cloned()
                .with_context(|| format!("{field_name} list unexpectedly empty"))
        }
        StringOrList::Empty => {
            bail!("Expected String or single-item List for {field_name}, got empty value")
        }
    }
}

fn parse_manifest_inner(path: &ManifestPath) {
    with_world(|world| match manifest::from_path(path.as_str()) {
        Ok(manifest) => {
            world.manifest.set_value(manifest);
            world.manifest_error.clear();
        }
        Err(e) => {
            world.manifest.clear_value();
            world.manifest_error.set(display_error_chain(e.as_ref()));
        }
    });
}

fn assert_manifest() -> Result<()> {
    with_world(|world| {
        ensure!(
            world.manifest.is_some(),
            "manifest should have been parsed successfully"
        );
        Ok(())
    })
}

fn assert_parsed() -> Result<()> {
    with_world(|world| {
        ensure!(
            world.manifest.is_some() || world.manifest_error.is_filled(),
            "manifest should have been parsed"
        );
        Ok(())
    })
}

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
    /// Create a location without an index.
    const fn new(context: &'static str, field: &'static str) -> Self {
        Self {
            context,
            index: None,
            field,
        }
    }

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
/// string values against expected typed values. Two variants are supported:
///
/// - `indexed`: For assertions that include an index (e.g., target N, macro N).
///   Generates: `fn $name(index: usize, actual: &str, expected: &$type) -> Result<()>`
///
/// - `simple`: For assertions without an index (e.g., first rule, manifest).
///   Generates: `fn $name(actual: &str, expected: &$type) -> Result<()>`
///
/// Both variants delegate to `assert_string_eq` with appropriate `FieldLocation`.
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
    (simple: $name:ident, $context:literal, $field:literal, $type:ty) => {
        fn $name(actual: &str, expected: &$type) -> Result<()> {
            assert_string_eq(FieldLocation::new($context, $field), actual, expected)
        }
    };
}

define_assertion!(indexed: assert_target_name_eq, "target", "name", TargetName);
define_assertion!(indexed: assert_target_command_eq, "target", "command", CommandText);
define_assertion!(indexed: assert_target_script_eq, "target", "script", ScriptText);
define_assertion!(indexed: assert_target_rule_eq, "target", "rule", RuleName);
define_assertion!(indexed: assert_macro_signature_eq, "macro", "signature", MacroSignature);
define_assertion!(simple: assert_rule_name_eq, "first rule", "name", RuleName);
define_assertion!(simple: assert_version_eq, "manifest", "version", VersionString);

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

fn parse_env_token<I>(chars: &mut std::iter::Peekable<I>) -> String
where
    I: Iterator<Item = char>,
{
    chars.next();
    let mut name = String::new();
    for ch in chars.by_ref() {
        if ch == '}' {
            break;
        }
        name.push(ch);
    }
    std::env::var(&name).unwrap_or_else(|_| ["${", &name, "}"].concat())
}

fn expand_env(raw: &str) -> String {
    let mut out = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' && chars.peek() == Some(&'{') {
            out.push_str(&parse_env_token(&mut chars));
        } else {
            out.push(c);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Generic helper functions for target access
// ---------------------------------------------------------------------------

/// Access a target by 1-based index and apply a closure to it.
fn with_target<T, F>(index: usize, f: F) -> Result<T>
where
    F: FnOnce(&netsuke::ast::Target) -> Result<T>,
{
    ensure!(index > 0, "target index is 1-based");
    with_world(|world| {
        let result = world.manifest.with_ref(|m| {
            let target = m
                .targets
                .get(index - 1)
                .with_context(|| format!("missing target {index}"))?;
            f(target)
        });
        with_manifest_error_context(result.context("manifest has not been parsed"))?
    })
}

/// Validate the phony flag on a target.
fn assert_target_phony(index: usize, expected: bool) -> Result<()> {
    with_target(index, |target| {
        ensure!(
            target.phony == expected,
            "target {index} phony should be {expected}"
        );
        Ok(())
    })
}

/// Validate the always flag on a target.
fn assert_target_always(index: usize, expected: bool) -> Result<()> {
    with_target(index, |target| {
        ensure!(
            target.always == expected,
            "target {index} always should be {expected}"
        );
        Ok(())
    })
}

/// Validate the number of targets in the manifest.
fn assert_target_count(expected: usize) -> Result<()> {
    with_world(|world| {
        let actual = world
            .manifest
            .with_ref(|m| m.targets.len())
            .context("manifest has not been parsed");
        let actual = with_manifest_error_context(actual)?;
        ensure!(
            actual == expected,
            "expected manifest to have {expected} targets, got {actual}"
        );
        Ok(())
    })
}

/// Validate the number of macros in the manifest.
fn assert_macro_count(expected: usize) -> Result<()> {
    with_world(|world| {
        let actual = world
            .manifest
            .with_ref(|m| m.macros.len())
            .context("manifest has not been parsed");
        let actual = with_manifest_error_context(actual)?;
        ensure!(
            actual == expected,
            "expected manifest to have {expected} macros, got {actual}"
        );
        Ok(())
    })
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
// Given steps
// ---------------------------------------------------------------------------

#[given("the environment variable {key} is set to {value}")]
fn set_env_var_step(key: String, value: String) -> Result<()> {
    let key = EnvVarKey::new(key);
    let value = EnvVarValue::new(value);
    ensure!(
        !key.as_str().is_empty(),
        "environment variable name must not be empty"
    );
    let expanded = expand_env(value.as_str());
    let previous = set_var(key.as_str(), OsStr::new(&expanded));
    with_world(|world| {
        world.track_env_var(key.into_string(), previous);
    });
    Ok(())
}

#[given("the environment variable {key} is unset")]
fn unset_env_var_step(key: String) -> Result<()> {
    let key = EnvVarKey::new(key);
    ensure!(
        !key.as_str().is_empty(),
        "environment variable name must not be empty"
    );
    let previous = remove_var(key.as_str());
    with_world(|world| {
        world.track_env_var(key.into_string(), previous);
    });
    Ok(())
}

#[given("the manifest file {path} is parsed")]
fn given_parse_manifest(path: String) -> Result<()> {
    let path = ManifestPath::new(path);
    ensure!(
        !path.as_str().trim().is_empty(),
        "manifest path must not be an empty string"
    );
    parse_manifest_inner(&path);
    Ok(())
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("the manifest file {path} is parsed")]
fn when_parse_manifest(path: String) -> Result<()> {
    let path = ManifestPath::new(path);
    ensure!(
        !path.as_str().trim().is_empty(),
        "manifest path must not be an empty string"
    );
    parse_manifest_inner(&path);
    Ok(())
}

#[when("the parsing result is checked")]
fn when_parsing_result_checked() -> Result<()> {
    assert_parsed()
}

#[when("the manifest is checked")]
fn when_manifest_checked() -> Result<()> {
    assert_manifest()
}

#[when("the version is checked")]
fn when_version_checked() -> Result<()> {
    assert_manifest()
}

#[when("the flags are checked")]
fn when_flags_checked() -> Result<()> {
    assert_manifest()
}

#[when("the rules are checked")]
fn when_rules_checked() -> Result<()> {
    assert_manifest()
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("the manifest version is {version}")]
fn manifest_version(version: String) -> Result<()> {
    let version = VersionString::new(version);
    with_world(|world| {
        let actual = world
            .manifest
            .with_ref(|m| m.netsuke_version.to_string())
            .context("manifest has not been parsed");
        let actual = with_manifest_error_context(actual)?;
        assert_version_eq(actual.as_str(), &version)
    })
}

#[then("the first target name is {name}")]
fn first_target_name(name: String) -> Result<()> {
    let name = TargetName::new(name);
    with_world(|world| {
        let result = world.manifest.with_ref(|m| {
            let target = m.targets.first().context("missing target 1")?;
            let actual = get_string_from_string_or_list(&target.name, "name")?;
            assert_target_name_eq(1, &actual, &name)
        });
        with_manifest_error_context(result.context("manifest has not been parsed"))?
    })
}

#[then("the target {index:usize} is phony")]
fn target_is_phony(index: usize) -> Result<()> {
    assert_target_phony(index, true)
}

#[then("the target {index:usize} is always rebuilt")]
fn target_is_always(index: usize) -> Result<()> {
    assert_target_always(index, true)
}

#[then("the target {index:usize} is not phony")]
fn target_not_phony(index: usize) -> Result<()> {
    assert_target_phony(index, false)
}

#[then("the target {index:usize} is not always rebuilt")]
fn target_not_always(index: usize) -> Result<()> {
    assert_target_always(index, false)
}

#[then("the first action is phony")]
fn first_action_phony() -> Result<()> {
    with_world(|world| {
        let result = world.manifest.with_ref(|m| {
            let first = m
                .actions
                .first()
                .context("manifest does not contain any actions")?;
            ensure!(first.phony, "expected first action to be marked phony");
            Ok(())
        });
        with_manifest_error_context(result.context("manifest has not been parsed"))?
    })
}

#[then("parsing the manifest fails")]
fn manifest_parse_error() -> Result<()> {
    with_world(|world| {
        ensure!(
            world.manifest_error.is_filled(),
            "expected manifest parsing to record an error"
        );
        Ok(())
    })
}

#[then("the error message contains {text}")]
fn manifest_error_contains(text: String) -> Result<()> {
    let text = ErrorPattern::new(text);
    with_world(|world| {
        let msg = world
            .manifest_error
            .get()
            .context("expected manifest parsing to produce an error")?;
        ensure!(
            msg.contains(text.as_str()),
            "expected parse error to contain '{}', but was '{msg}'",
            text
        );
        Ok(())
    })
}

#[then("the first rule name is {name}")]
fn first_rule_name(name: String) -> Result<()> {
    let name = RuleName::new(name);
    with_world(|world| {
        let result = world.manifest.with_ref(|m| {
            let rule = m
                .rules
                .first()
                .context("manifest does not contain any rules")?;
            assert_rule_name_eq(rule.name.as_str(), &name)
        });
        with_manifest_error_context(result.context("manifest has not been parsed"))?
    })
}

#[then("the first target command is {command}")]
fn first_target_command(command: String) -> Result<()> {
    let command = CommandText::new(command);
    with_world(|world| {
        let result = world.manifest.with_ref(|m| {
            let target = m.targets.first().context("missing target 1")?;
            match &target.recipe {
                Recipe::Command { command: actual } => {
                    assert_target_command_eq(1, actual, &command)
                }
                other => bail!("Expected command recipe, got: {other:?}"),
            }
        });
        with_manifest_error_context(result.context("manifest has not been parsed"))?
    })
}

#[then("the manifest has {count:usize} targets")]
fn manifest_has_targets(count: usize) -> Result<()> {
    assert_target_count(count)
}

#[then("the manifest has {count:usize} macros")]
fn manifest_has_macros(count: usize) -> Result<()> {
    assert_macro_count(count)
}

#[then("the macro {index:usize} signature is {signature}")]
fn macro_signature_is(index: usize, signature: String) -> Result<()> {
    let signature = MacroSignature::new(signature);
    ensure!(index > 0, "macros use 1-based index");
    with_world(|world| {
        let result = world.manifest.with_ref(|m| {
            let macro_def = m
                .macros
                .get(index - 1)
                .with_context(|| format!("missing macro {index}"))?;
            assert_macro_signature_eq(index, macro_def.signature.as_str(), &signature)
        });
        with_manifest_error_context(result.context("manifest has not been parsed"))?
    })
}

#[when("the manifest has targets named {names}")]
#[then("the manifest has targets named {names}")]
fn manifest_has_targets_named(names: String) -> Result<()> {
    let names = NamesList::new(names);
    let expected: BTreeSet<String> = names.to_set();
    with_world(|world| {
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
        with_manifest_error_context(result.context("manifest has not been parsed"))?
    })
}

#[then("the target {index:usize} name is {name}")]
fn target_name_n(index: usize, name: String) -> Result<()> {
    let name = TargetName::new(name);
    with_target(index, |target| {
        let actual = get_string_from_string_or_list(&target.name, "name")?;
        assert_target_name_eq(index, &actual, &name)
    })
}

#[then("the target {index:usize} command is {command}")]
fn target_command_n(index: usize, command: String) -> Result<()> {
    let command = CommandText::new(command);
    with_target(index, |target| match &target.recipe {
        Recipe::Command { command: actual } => assert_target_command_eq(index, actual, &command),
        other => bail!("Expected command recipe, got: {other:?}"),
    })
}

#[then("the target {index:usize} index is {expected:usize}")]
fn target_index_n(index: usize, expected: usize) -> Result<()> {
    with_target(index, |target| {
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

#[then("the target {index:usize} has source {source}")]
fn target_has_source(index: usize, source: String) -> Result<()> {
    let source = SourcePath::new(source);
    with_target(index, |target| {
        assert_target_has_source(index, &target.sources, &source)
    })
}

#[then("the target {index:usize} has dep {dep}")]
fn target_has_dep(index: usize, dep: String) -> Result<()> {
    let dep = DepName::new(dep);
    with_target(index, |target| {
        assert_target_has_dep(index, &target.deps, &dep)
    })
}

#[then("the target {index:usize} has order-only dep {dep}")]
fn target_has_order_only_dep(index: usize, dep: String) -> Result<()> {
    let dep = DepName::new(dep);
    with_target(index, |target| {
        assert_target_has_order_only_dep(index, &target.order_only_deps, &dep)
    })
}

#[then("the target {index:usize} script is {script}")]
fn target_script_is(index: usize, script: String) -> Result<()> {
    let script = ScriptText::new(script);
    with_target(index, |target| match &target.recipe {
        Recipe::Script { script: actual } => assert_target_script_eq(index, actual, &script),
        other => bail!("Expected script recipe, got: {other:?}"),
    })
}

#[then("the target {index:usize} rule is {rule}")]
fn target_rule_is(index: usize, rule: String) -> Result<()> {
    let rule = RuleName::new(rule);
    with_target(index, |target| match &target.recipe {
        Recipe::Rule { rule: actual } => {
            let actual_str = get_string_from_string_or_list(actual, "rule")?;
            assert_target_rule_eq(index, &actual_str, &rule)
        }
        other => bail!("Expected rule recipe, got: {other:?}"),
    })
}
