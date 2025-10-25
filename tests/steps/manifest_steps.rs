//! Step definitions for manifest feature tests.
#![expect(
    clippy::shadow_reuse,
    reason = "Cucumber step macros rebind capture names"
)]
use crate::CliWorld;
use anyhow::{Context, Result, bail, ensure};
use cucumber::{given, then, when};
use netsuke::{
    ast::{Recipe, StringOrList, Target},
    manifest,
};
use std::{collections::BTreeSet, convert::TryFrom, ffi::OsStr};
use test_support::display_error_chain;
use test_support::env::{remove_var, set_var};

const INDEX_KEY: &str = "index";

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

fn parse_manifest_inner(world: &mut CliWorld, path: &str) {
    match manifest::from_path(path) {
        Ok(manifest) => {
            world.manifest = Some(manifest);
            world.manifest_error = None;
        }
        Err(e) => {
            world.manifest = None;
            // Record the error chain using `Display` for stable substring checks.
            world.manifest_error = Some(display_error_chain(e.as_ref()));
        }
    }
}

fn assert_manifest(world: &CliWorld) -> Result<()> {
    ensure!(
        world.manifest.is_some(),
        "manifest should have been parsed successfully"
    );
    Ok(())
}

fn assert_parsed(world: &CliWorld) -> Result<()> {
    ensure!(
        world.manifest.is_some() || world.manifest_error.is_some(),
        "manifest should have been parsed"
    );
    Ok(())
}

fn get_target(world: &CliWorld, index: usize) -> Result<&Target> {
    ensure!(index > 0, "target index is 1-based");
    let manifest = manifest(world)?;
    manifest
        .targets
        .get(index - 1)
        .with_context(|| format!("missing target {index}"))
}

fn manifest(world: &CliWorld) -> Result<&netsuke::ast::NetsukeManifest> {
    world
        .manifest
        .as_ref()
        .context("manifest has not been parsed")
}

fn assert_manifest_collection_len(
    world: &CliWorld,
    collection_name: &str,
    actual_len: usize,
    expected_len: usize,
) -> Result<()> {
    debug_assert!(
        world.manifest.is_some(),
        "manifest should be parsed before asserting {collection_name}"
    );
    ensure!(
        actual_len == expected_len,
        "expected manifest to have {expected_len} {collection_name}, got {actual_len}"
    );
    Ok(())
}

fn assert_field_eq(
    context_name: &str,
    field_name: &str,
    actual: &str,
    expected: &str,
) -> Result<()> {
    ensure!(
        actual == expected,
        "expected {context_name} {field_name} '{expected}', got '{actual}'"
    );
    Ok(())
}

fn parse_env_token<I>(chars: &mut std::iter::Peekable<I>) -> String
where
    I: Iterator<Item = char>,
{
    // Preserve the token if the variable is unset.
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

#[given(expr = "the environment variable {string} is set to {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn set_env_var(world: &mut CliWorld, key: String, value: String) -> Result<()> {
    // Central helper acquires the global lock and returns the prior value so
    // the scenario can restore it afterwards.
    ensure!(
        !key.is_empty(),
        "environment variable name must not be empty"
    );
    let expanded = expand_env(&value);
    let previous = set_var(&key, OsStr::new(&expanded));
    world.env_vars.entry(key).or_insert(previous);
    Ok(())
}

#[given(expr = "the environment variable {string} is unset")]
fn unset_env_var(world: &mut CliWorld, key: String) -> Result<()> {
    // Capture any previous value for restoration when the scenario ends.
    ensure!(
        !key.is_empty(),
        "environment variable name must not be empty"
    );
    let previous = remove_var(&key);
    world.env_vars.entry(key).or_insert(previous);
    Ok(())
}

#[given(expr = "the manifest file {string} is parsed")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn given_parse_manifest(world: &mut CliWorld, path: String) -> Result<()> {
    ensure!(
        !path.trim().is_empty(),
        "manifest path must not be an empty string"
    );
    parse_manifest_inner(world, &path);
    Ok(())
}

#[when(expr = "the manifest file {string} is parsed")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn parse_manifest(world: &mut CliWorld, path: String) -> Result<()> {
    ensure!(
        !path.trim().is_empty(),
        "manifest path must not be an empty string"
    );
    parse_manifest_inner(world, &path);
    Ok(())
}

#[when(regex = r"^the (?P<item>parsing result|manifest|version|flags|rules) (?:is|are) checked$")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn when_item_checked(world: &mut CliWorld, item: String) -> Result<()> {
    match item.as_str() {
        "parsing result" => assert_parsed(world)?,
        "manifest" | "version" | "flags" | "rules" => assert_manifest(world)?,
        unexpected => bail!("Unexpected item checked: '{unexpected}'"),
    }
    Ok(())
}

#[then(expr = "the manifest version is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn manifest_version(world: &mut CliWorld, version: String) -> Result<()> {
    let manifest = manifest(world)?;
    let actual = manifest.netsuke_version.to_string();
    assert_field_eq("manifest", "version", actual.as_str(), version.as_str())
}

#[then(expr = "the first target name is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn first_target_name(world: &mut CliWorld, name: String) -> Result<()> {
    assert_target_name(world, 1, &name)
}

#[then(expr = "the target {int} is phony")]
fn target_is_phony(world: &mut CliWorld, index: usize) -> Result<()> {
    let target = get_target(world, index)?;
    ensure!(target.phony, "target {index} should be phony");
    Ok(())
}

#[then(expr = "the target {int} is always rebuilt")]
fn target_is_always(world: &mut CliWorld, index: usize) -> Result<()> {
    let target = get_target(world, index)?;
    ensure!(target.always, "target {index} should always build");
    Ok(())
}

#[then(expr = "the target {int} is not phony")]
fn target_not_phony(world: &mut CliWorld, index: usize) -> Result<()> {
    let target = get_target(world, index)?;
    ensure!(!target.phony, "target {index} should not be phony");
    Ok(())
}

#[then(expr = "the target {int} is not always rebuilt")]
fn target_not_always(world: &mut CliWorld, index: usize) -> Result<()> {
    let target = get_target(world, index)?;
    ensure!(!target.always, "target {index} should not always build");
    Ok(())
}

#[then("the first action is phony")]
fn first_action_phony(world: &mut CliWorld) -> Result<()> {
    let manifest = manifest(world)?;
    let first = manifest
        .actions
        .first()
        .context("manifest does not contain any actions")?;
    ensure!(first.phony, "expected first action to be marked phony");
    Ok(())
}

#[then("parsing the manifest fails")]
fn manifest_parse_error(world: &mut CliWorld) -> Result<()> {
    ensure!(
        world.manifest_error.is_some(),
        "expected manifest parsing to record an error"
    );
    Ok(())
}

#[then(expr = "the error message contains {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn manifest_error_contains(world: &mut CliWorld, text: String) -> Result<()> {
    let msg = world
        .manifest_error
        .as_ref()
        .context("expected manifest parsing to produce an error")?;
    ensure!(
        msg.contains(&text),
        "expected parse error to contain '{text}', but was '{msg}'"
    );
    Ok(())
}

#[then(expr = "the first rule name is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn first_rule_name(world: &mut CliWorld, name: String) -> Result<()> {
    let manifest = manifest(world)?;
    let rule = manifest
        .rules
        .first()
        .context("manifest does not contain any rules")?;
    assert_field_eq("first rule", "name", rule.name.as_str(), name.as_str())
}

#[then(expr = "the first target command is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn first_target_command(world: &mut CliWorld, command: String) -> Result<()> {
    assert_target_command(world, 1, &command)
}

#[then(expr = "the manifest has {int} targets")]
fn manifest_has_targets(world: &mut CliWorld, count: usize) -> Result<()> {
    let manifest = manifest(world)?;
    let actual = manifest.targets.len();
    assert_manifest_collection_len(world, "targets", actual, count)
}

#[then(expr = "the manifest has {int} macros")]
fn manifest_has_macros(world: &mut CliWorld, count: usize) -> Result<()> {
    let manifest = manifest(world)?;
    let actual = manifest.macros.len();
    assert_manifest_collection_len(world, "macros", actual, count)
}

#[then(expr = "the macro {int} signature is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn macro_signature_is(world: &mut CliWorld, index: usize, signature: String) -> Result<()> {
    ensure!(index > 0, "macros use 1-based index");
    let manifest = manifest(world)?;
    let macro_def = manifest
        .macros
        .get(index - 1)
        .with_context(|| format!("missing macro {index}"))?;
    assert_field_eq(
        &format!("macro {index}"),
        "signature",
        macro_def.signature.as_str(),
        signature.as_str(),
    )
}

#[then(expr = "the manifest has targets named {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn manifest_has_targets_named(world: &mut CliWorld, names: String) -> Result<()> {
    let expected: BTreeSet<String> = names
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    let manifest = manifest(world)?;
    let actual: BTreeSet<String> = manifest
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
}

fn assert_target_name(world: &CliWorld, index: usize, name: &str) -> Result<()> {
    let target = get_target(world, index)?;
    let actual = get_string_from_string_or_list(&target.name, "name")?;
    ensure!(
        actual == name,
        "expected target {index} name '{name}', got '{actual}'"
    );
    Ok(())
}

fn assert_target_command(world: &CliWorld, index: usize, command: &str) -> Result<()> {
    let target = get_target(world, index)?;
    match &target.recipe {
        Recipe::Command { command: actual } => {
            assert_field_eq(&format!("target {index}"), "command", actual, command)
        }
        other => bail!("Expected command recipe, got: {other:?}"),
    }
}

fn assert_target_index(world: &CliWorld, index: usize, expected: usize) -> Result<()> {
    let target = get_target(world, index)?;
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

#[then(expr = "the target {int} name is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn target_name_n(world: &mut CliWorld, index: usize, name: String) -> Result<()> {
    assert_target_name(world, index, &name)
}

#[then(expr = "the target {int} command is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn target_command_n(world: &mut CliWorld, index: usize, command: String) -> Result<()> {
    assert_target_command(world, index, &command)
}

#[then(expr = "the target {int} index is {int}")]
fn target_index_n(world: &mut CliWorld, index: usize, expected: usize) -> Result<()> {
    assert_target_index(world, index, expected)
}

#[then(expr = "the target {int} has source {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn target_has_source(world: &mut CliWorld, index: usize, source: String) -> Result<()> {
    let target = get_target(world, index)?;
    assert_list_contains(&target.sources, &source)
}

#[then(expr = "the target {int} has dep {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn target_has_dep(world: &mut CliWorld, index: usize, dep: String) -> Result<()> {
    let target = get_target(world, index)?;
    assert_list_contains(&target.deps, &dep)
}

#[then(expr = "the target {int} has order-only dep {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn target_has_order_only_dep(
    world: &mut CliWorld,
    target_index: usize,
    expected_dep: String,
) -> Result<()> {
    let target = get_target(world, target_index)?;
    assert_list_contains(&target.order_only_deps, &expected_dep)
}

#[then(expr = "the target {int} script is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn target_script_is(
    world: &mut CliWorld,
    target_index: usize,
    expected_script: String,
) -> Result<()> {
    let target = get_target(world, target_index)?;
    match &target.recipe {
        Recipe::Script { script: actual } => {
            ensure!(
                actual == &expected_script,
                "expected target {target_index} script '{expected_script}', got '{actual}'"
            );
            Ok(())
        }
        other => bail!("Expected script recipe, got: {other:?}"),
    }
}

#[then(expr = "the target {int} rule is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn target_rule_is(
    world: &mut CliWorld,
    target_index: usize,
    expected_rule_name: String,
) -> Result<()> {
    let target = get_target(world, target_index)?;
    match &target.recipe {
        Recipe::Rule { rule } => {
            let actual = get_string_from_string_or_list(rule, "rule")?;
            ensure!(
                actual == expected_rule_name,
                "expected target {target_index} rule '{expected_rule_name}', got '{actual}'"
            );
            Ok(())
        }
        other => bail!("Expected rule recipe, got: {other:?}"),
    }
}
