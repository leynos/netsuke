//! Step definitions for manifest feature tests.
#![allow(
    clippy::shadow_reuse,
    clippy::shadow_unrelated,
    reason = "Cucumber step macros rebind capture names"
)]

use crate::CliWorld;
use cucumber::{given, then, when};
use netsuke::{
    ast::{Recipe, StringOrList, Target},
    manifest::{self, ManifestValue},
};
use std::collections::BTreeSet;
use std::ffi::OsStr;
use test_support::display_error_chain;
use test_support::env::{remove_var, set_var};

const INDEX_KEY: &str = "index";

fn get_string_from_string_or_list(value: &StringOrList, field_name: &str) -> String {
    match value {
        StringOrList::String(s) => s.clone(),
        StringOrList::List(list) if list.len() == 1 => list.first().expect("one element").clone(),
        other => panic!("Expected String or single-item List for {field_name}, got: {other:?}"),
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

fn assert_manifest(world: &CliWorld) {
    assert!(
        world.manifest.is_some(),
        "manifest should have been parsed successfully",
    );
}

fn assert_parsed(world: &CliWorld) {
    assert!(
        world.manifest.is_some() || world.manifest_error.is_some(),
        "manifest should have been parsed",
    );
}

fn get_target(world: &CliWorld, index: usize) -> &Target {
    let manifest = world.manifest.as_ref().expect("manifest");
    let idx0 = index.checked_sub(1).expect("target index is 1-based");
    manifest
        .targets
        .get(idx0)
        .unwrap_or_else(|| panic!("missing target {index}"))
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
fn set_env_var(world: &mut CliWorld, key: String, value: String) {
    // Central helper acquires the global lock and returns the prior value so
    // the scenario can restore it afterwards.
    let expanded = expand_env(&value);
    let previous = set_var(&key, OsStr::new(&expanded));
    world.env_vars.entry(key).or_insert(previous);
}

#[given(expr = "the environment variable {string} is unset")]
fn unset_env_var(world: &mut CliWorld, key: String) {
    // Capture any previous value for restoration when the scenario ends.
    let previous = remove_var(&key);
    world.env_vars.entry(key).or_insert(previous);
}

#[given(expr = "the manifest file {string} is parsed")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn given_parse_manifest(world: &mut CliWorld, path: String) {
    parse_manifest_inner(world, &path);
}

#[when(expr = "the manifest file {string} is parsed")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn parse_manifest(world: &mut CliWorld, path: String) {
    parse_manifest_inner(world, &path);
}

#[when(regex = r"^the (?P<item>parsing result|manifest|version|flags|rules) (?:is|are) checked$")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn when_item_checked(world: &mut CliWorld, item: String) {
    match item.as_str() {
        "parsing result" => assert_parsed(world),
        "manifest" | "version" | "flags" | "rules" => assert_manifest(world),
        unexpected => panic!("Unexpected item checked: '{unexpected}'"),
    }
}

#[then(expr = "the manifest version is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn manifest_version(world: &mut CliWorld, version: String) {
    let manifest = world.manifest.as_ref().expect("manifest");
    assert_eq!(manifest.netsuke_version.to_string(), version);
}

#[then(expr = "the first target name is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn first_target_name(world: &mut CliWorld, name: String) {
    assert_target_name(world, 1, &name);
}

#[then(expr = "the target {int} is phony")]
fn target_is_phony(world: &mut CliWorld, index: usize) {
    let target = get_target(world, index);
    assert!(target.phony);
}

#[then(expr = "the target {int} is always rebuilt")]
fn target_is_always(world: &mut CliWorld, index: usize) {
    let target = get_target(world, index);
    assert!(target.always);
}

#[then(expr = "the target {int} is not phony")]
fn target_not_phony(world: &mut CliWorld, index: usize) {
    let target = get_target(world, index);
    assert!(!target.phony);
}

#[then(expr = "the target {int} is not always rebuilt")]
fn target_not_always(world: &mut CliWorld, index: usize) {
    let target = get_target(world, index);
    assert!(!target.always);
}

#[then("the first action is phony")]
fn first_action_phony(world: &mut CliWorld) {
    let manifest = world.manifest.as_ref().expect("manifest");
    let first = manifest.actions.first().expect("actions");
    assert!(first.phony);
}

#[then("parsing the manifest fails")]
fn manifest_parse_error(world: &mut CliWorld) {
    assert!(world.manifest_error.is_some(), "expected parse error");
}

#[then(expr = "the error message contains {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn manifest_error_contains(world: &mut CliWorld, text: String) {
    let msg = world.manifest_error.as_ref().expect("expected parse error");
    assert!(msg.contains(&text), "{msg}");
}

#[then(expr = "the first rule name is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn first_rule_name(world: &mut CliWorld, name: String) {
    let manifest = world.manifest.as_ref().expect("manifest");
    let rule = manifest.rules.first().expect("rules");
    assert_eq!(rule.name, name);
}

#[then(expr = "the first target command is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn first_target_command(world: &mut CliWorld, command: String) {
    assert_target_command(world, 1, &command);
}

#[then(expr = "the manifest has {int} targets")]
fn manifest_has_targets(world: &mut CliWorld, count: usize) {
    let manifest = world.manifest.as_ref().expect("manifest");
    assert_eq!(manifest.targets.len(), count);
}

#[then(expr = "the manifest has {int} macros")]
fn manifest_has_macros(world: &mut CliWorld, count: usize) {
    let manifest = world.manifest.as_ref().expect("manifest");
    assert_eq!(manifest.macros.len(), count);
}

#[then(expr = "the macro {int} signature is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn macro_signature_is(world: &mut CliWorld, index: usize, signature: String) {
    let manifest = world.manifest.as_ref().expect("manifest");
    let idx = index.checked_sub(1).expect("macros use 1-based index");
    let macro_def = manifest
        .macros
        .get(idx)
        .unwrap_or_else(|| panic!("missing macro {index}"));
    assert_eq!(macro_def.signature, signature);
}

#[then(expr = "the manifest has targets named {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn manifest_has_targets_named(world: &mut CliWorld, names: String) {
    let expected: BTreeSet<String> = names
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    let manifest = world.manifest.as_ref().expect("manifest");
    let actual: BTreeSet<String> = manifest
        .targets
        .iter()
        .map(|t| get_string_from_string_or_list(&t.name, "name"))
        .collect();
    let missing: BTreeSet<_> = expected.difference(&actual).cloned().collect();
    let extra: BTreeSet<_> = actual.difference(&expected).cloned().collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "target names differ\nmissing: {missing:?}\nextra: {extra:?}",
    );
}

fn assert_target_name(world: &CliWorld, index: usize, name: &str) {
    let target = get_target(world, index);
    let actual = get_string_from_string_or_list(&target.name, "name");
    assert_eq!(&actual, name);
}

fn assert_target_command(world: &CliWorld, index: usize, command: &str) {
    let target = get_target(world, index);
    if let Recipe::Command { command: actual } = &target.recipe {
        assert_eq!(actual, command);
    } else {
        panic!("Expected command recipe, got: {:?}", target.recipe);
    }
}

fn assert_target_index(world: &CliWorld, index: usize, expected: usize) {
    let target = get_target(world, index);
    let actual = target
        .vars
        .get(INDEX_KEY)
        .and_then(ManifestValue::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or_else(|| panic!("target {index} missing index"));
    assert_eq!(actual, expected, "unexpected index for target {index}");
}

fn assert_list_contains(value: &StringOrList, expected: &str) {
    match value {
        StringOrList::List(list) => {
            assert!(list.contains(&expected.to_owned()), "missing {expected}");
        }
        StringOrList::String(s) => {
            assert_eq!(s, expected);
        }
        StringOrList::Empty => panic!("value is empty"),
    }
}

#[then(expr = "the target {int} name is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn target_name_n(world: &mut CliWorld, index: usize, name: String) {
    assert_target_name(world, index, &name);
}

#[then(expr = "the target {int} command is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn target_command_n(world: &mut CliWorld, index: usize, command: String) {
    assert_target_command(world, index, &command);
}

#[then(expr = "the target {int} index is {int}")]
fn target_index_n(world: &mut CliWorld, index: usize, expected: usize) {
    assert_target_index(world, index, expected);
}

#[then(expr = "the target {int} has source {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn target_has_source(world: &mut CliWorld, index: usize, source: String) {
    let target = get_target(world, index);
    assert_list_contains(&target.sources, &source);
}

#[then(expr = "the target {int} has dep {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn target_has_dep(world: &mut CliWorld, index: usize, dep: String) {
    let target = get_target(world, index);
    assert_list_contains(&target.deps, &dep);
}

#[then(expr = "the target {int} has order-only dep {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn target_has_order_only_dep(world: &mut CliWorld, target_index: usize, expected_dep: String) {
    let target = get_target(world, target_index);
    assert_list_contains(&target.order_only_deps, &expected_dep);
}

#[then(expr = "the target {int} script is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn target_script_is(world: &mut CliWorld, target_index: usize, expected_script: String) {
    let target = get_target(world, target_index);
    if let Recipe::Script { script: actual } = &target.recipe {
        assert_eq!(actual, &expected_script);
    } else {
        panic!("Expected script recipe, got: {:?}", target.recipe);
    }
}

#[then(expr = "the target {int} rule is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn target_rule_is(world: &mut CliWorld, target_index: usize, expected_rule_name: String) {
    let target = get_target(world, target_index);
    if let Recipe::Rule { rule } = &target.recipe {
        let actual = get_string_from_string_or_list(rule, "rule");
        assert_eq!(actual, expected_rule_name);
    } else {
        panic!("Expected rule recipe, got: {:?}", target.recipe);
    }
}
