//! Step definitions for manifest parsing scenarios.

use crate::CliWorld;
use cucumber::{given, then, when};
use netsuke::{ast::StringOrList, manifest};

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
        "manifest should have been parsed successfully"
    );
}

fn assert_parsed(world: &CliWorld) {
    assert!(
        world.manifest.is_some() || world.manifest_error.is_some(),
        "manifest should have been parsed"
    );
}
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[given(expr = "the manifest file {string} is parsed")]
fn given_parse_manifest(world: &mut CliWorld, path: String) {
    parse_manifest_inner(world, &path);
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[when(expr = "the manifest file {string} is parsed")]
fn parse_manifest(world: &mut CliWorld, path: String) {
    parse_manifest_inner(world, &path);
}

#[when("the manifest version is checked")]
fn when_manifest_version_checked(world: &mut CliWorld) {
    assert_manifest(world);
}

#[when("the target flags are checked")]
fn when_target_flags_checked(world: &mut CliWorld) {
    assert_manifest(world);
}

#[when("the action flags are checked")]
fn when_action_flags_checked(world: &mut CliWorld) {
    assert_manifest(world);
}

#[when("the manifest contents are checked")]
fn when_manifest_contents_checked(world: &mut CliWorld) {
    assert_manifest(world);
}

#[when("the parsing result is checked")]
fn when_parsing_result_checked(world: &mut CliWorld) {
    assert_parsed(world);
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "the manifest version is {string}")]
fn manifest_version(world: &mut CliWorld, version: String) {
    let manifest = world.manifest.as_ref().expect("manifest");
    assert_eq!(manifest.netsuke_version.to_string(), version);
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "the first target name is {string}")]
fn first_target_name(world: &mut CliWorld, name: String) {
    let manifest = world.manifest.as_ref().expect("manifest");
    let first = manifest.targets.first().expect("targets");
    match &first.name {
        StringOrList::String(value) => assert_eq!(value, &name),
        other => panic!("Expected StringOrList::String, got: {other:?}"),
    }
}

#[then("the first target is phony")]
fn first_target_phony(world: &mut CliWorld) {
    let manifest = world.manifest.as_ref().expect("manifest");
    let first = manifest.targets.first().expect("targets");
    assert!(first.phony);
}

#[then("the first target is always rebuilt")]
fn first_target_always(world: &mut CliWorld) {
    let manifest = world.manifest.as_ref().expect("manifest");
    let first = manifest.targets.first().expect("targets");
    assert!(first.always);
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

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "the first rule name is {string}")]
fn first_rule_name(world: &mut CliWorld, name: String) {
    let manifest = world.manifest.as_ref().expect("manifest");
    let rule = manifest.rules.first().expect("rules");
    assert_eq!(rule.name, name);
}
