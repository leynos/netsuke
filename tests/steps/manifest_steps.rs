//! Step definitions for manifest feature tests.

use crate::CliWorld;
use cucumber::{given, then, when};
use netsuke::{
    ast::{Recipe, StringOrList},
    manifest,
};

fn parse_manifest_inner(world: &mut CliWorld, path: &str) {
    match manifest::from_path(path) {
        Ok(manifest) => {
            world.manifest = Some(manifest);
            world.manifest_error = None;
        }
        Err(e) => {
            world.manifest = None;
            world.manifest_error = Some(e.to_string());
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
    let manifest = world.manifest.as_ref().expect("manifest");
    let target = manifest
        .targets
        .get(index - 1)
        .unwrap_or_else(|| panic!("missing target {index}"));
    assert!(target.phony);
}

#[then(expr = "the target {int} is always rebuilt")]
fn target_is_always(world: &mut CliWorld, index: usize) {
    let manifest = world.manifest.as_ref().expect("manifest");
    let target = manifest
        .targets
        .get(index - 1)
        .unwrap_or_else(|| panic!("missing target {index}"));
    assert!(target.always);
}

#[then(expr = "the target {int} is not phony")]
fn target_not_phony(world: &mut CliWorld, index: usize) {
    let manifest = world.manifest.as_ref().expect("manifest");
    let target = manifest
        .targets
        .get(index - 1)
        .unwrap_or_else(|| panic!("missing target {index}"));
    assert!(!target.phony);
}

#[then(expr = "the target {int} is not always rebuilt")]
fn target_not_always(world: &mut CliWorld, index: usize) {
    let manifest = world.manifest.as_ref().expect("manifest");
    let target = manifest
        .targets
        .get(index - 1)
        .unwrap_or_else(|| panic!("missing target {index}"));
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

fn assert_target_name(world: &CliWorld, index: usize, name: &str) {
    let manifest = world.manifest.as_ref().expect("manifest");
    let target = manifest
        .targets
        .get(index - 1)
        .unwrap_or_else(|| panic!("missing target {index}"));
    match &target.name {
        StringOrList::String(value) => assert_eq!(value, name),
        other => panic!("Expected StringOrList::String, got: {other:?}"),
    }
}

fn assert_target_command(world: &CliWorld, index: usize, command: &str) {
    let manifest = world.manifest.as_ref().expect("manifest");
    let target = manifest
        .targets
        .get(index - 1)
        .unwrap_or_else(|| panic!("missing target {index}"));
    if let Recipe::Command { command: actual } = &target.recipe {
        assert_eq!(actual, command);
    } else {
        panic!("Expected command recipe, got: {:?}", target.recipe);
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
