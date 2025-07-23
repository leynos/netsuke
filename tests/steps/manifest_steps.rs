//! Step definitions for manifest parsing scenarios.

use crate::CliWorld;
use cucumber::{then, when};
use netsuke::ast::{NetsukeManifest, StringOrList};
use std::fs;

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[when(expr = "the manifest file {string} is parsed")]
fn parse_manifest(world: &mut CliWorld, path: String) {
    let yaml = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(e) => {
            world.manifest = None;
            world.manifest_error = Some(format!("Failed to read {path}: {e}"));
            return;
        }
    };
    match serde_yml::from_str::<NetsukeManifest>(&yaml) {
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

#[then("the first step is phony")]
fn first_step_phony(world: &mut CliWorld) {
    let manifest = world.manifest.as_ref().expect("manifest");
    let first = manifest.steps.first().expect("steps");
    assert!(first.phony);
}

#[then("parsing the manifest fails")]
fn manifest_parse_error(world: &mut CliWorld) {
    assert!(world.manifest_error.is_some(), "expected parse error");
}
