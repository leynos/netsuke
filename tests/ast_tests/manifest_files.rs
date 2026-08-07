//! Tests that load manifests from `tests/data/` through `manifest::from_path`,
//! covering the on-disk entry point rather than the in-memory parser.

use anyhow::{Context, Result, bail, ensure};
use netsuke::{ast::StringOrList, manifest};
use rstest::rstest;
use semver::Version;

#[test]
fn load_manifest_from_file() -> Result<()> {
    let manifest = manifest::from_path("tests/data/minimal.yml")?;
    let expected_version = Version::parse("1.0.0")?;
    ensure!(
        manifest.netsuke_version == expected_version,
        "unexpected manifest version: got {}, expected {}",
        manifest.netsuke_version,
        expected_version
    );
    Ok(())
}

#[test]
fn load_manifest_missing_file() {
    let result = manifest::from_path("tests/data/missing.yml");
    assert!(result.is_err(), "absent manifest path should fail to load");
}

#[rstest]
#[case("minimal.yml", "hello")]
#[case("phony.yml", "clean")]
#[case("rules.yml", "hello.o")]
fn parse_example_manifests(#[case] file: &str, #[case] first_target: &str) -> Result<()> {
    let path = format!("tests/data/{file}");
    let manifest = manifest::from_path(&path)?;
    let first = manifest
        .targets
        .first()
        .context("expected target entry in manifest")?;
    match &first.name {
        StringOrList::String(name) => {
            ensure!(name == first_target, "unexpected name: {name}");
        }
        other => bail!("Expected String variant, got: {other:?}"),
    }
    Ok(())
}

#[rstest]
#[case("unknown_field.yml")]
#[case("invalid_version.yml")]
#[case("missing_recipe.yml")]
#[case("action_invalid.yml")]
fn invalid_manifests_fail(#[case] file: &str) {
    let path = format!("tests/data/{file}");
    assert!(
        manifest::from_path(&path).is_err(),
        "{file} should fail to parse"
    );
}
