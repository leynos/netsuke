//! Step definitions for manifest parsing scenarios.

use crate::CliWorld;
use cucumber::{then, when};
use netsuke::ast::{NetsukeManifest, StringOrList};
use std::fs;

#[when(expr = "the manifest file {string} is parsed")]
fn parse_manifest(world: &mut CliWorld, path: String) {
    let yaml = fs::read_to_string(path).expect("read manifest");
    match serde_yaml::from_str::<NetsukeManifest>(&yaml) {
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
    assert_eq!(manifest.netsuke_version, version);
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
        other => panic!("unexpected variant {other:?}"),
    }
}
