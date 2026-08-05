//! Contract tests for the `cargo binstall` metadata in `Cargo.toml`.
//!
//! The README and the users' guide advertise `cargo binstall netsuke-build`.
//! `cargo binstall` derives its default asset names from the Cargo package
//! name, but every release asset is named after the `netsuke` binary, so the
//! package needs explicit `[package.metadata.binstall]` overrides. These tests
//! hold those overrides to `.github/release-staging.toml` and to the release
//! workflow's target matrix so a packaging change cannot silently strand the
//! documented command on a source build.

use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::collections::BTreeMap;
use test_support::fs as test_fs;

const CARGO_MANIFEST: &str = "Cargo.toml";
const STAGING_CONFIG: &str = ".github/release-staging.toml";
const RELEASE_WORKFLOW: &str = ".github/workflows/release.yml";

/// Workflow artefact names that prefix each uploaded release asset.
///
/// `release.yml` builds these as `<repository name>-<matrix suffix>`, which is
/// not derivable from `.github/release-staging.toml`. The staging-target keys
/// are checked against the workflow matrix below, so a matrix rename fails
/// here rather than at release time.
const WORKFLOW_ARTEFACT_NAMES: [(&str, &str); 6] = [
    ("linux-x86_64", "netsuke-linux-amd64"),
    ("linux-aarch64", "netsuke-linux-arm64"),
    ("windows-x86_64", "netsuke-windows-amd64"),
    ("windows-aarch64", "netsuke-windows-arm64"),
    ("macos-x86_64", "netsuke-macos-x86_64"),
    ("macos-aarch64", "netsuke-macos-arm64"),
];

#[derive(Deserialize)]
struct StagingConfig {
    common: StagingCommon,
    targets: BTreeMap<String, StagingTarget>,
}

#[derive(Deserialize)]
struct StagingCommon {
    bin_name: String,
    staging_dir_template: String,
}

#[derive(Deserialize)]
struct StagingTarget {
    platform: String,
    arch: String,
    target: String,
    #[serde(default)]
    bin_ext: String,
}

#[derive(Deserialize)]
struct CargoManifest {
    package: CargoPackage,
}

#[derive(Deserialize)]
struct CargoPackage {
    name: String,
    metadata: CargoPackageMetadata,
}

#[derive(Deserialize)]
struct CargoPackageMetadata {
    binstall: BinstallMetadata,
}

#[derive(Deserialize)]
struct BinstallMetadata {
    #[serde(rename = "pkg-fmt")]
    pkg_fmt: String,
    #[serde(default)]
    overrides: BTreeMap<String, BinstallOverride>,
}

#[derive(Deserialize)]
struct BinstallOverride {
    #[serde(rename = "pkg-url")]
    pkg_url: String,
}

fn load<T: serde::de::DeserializeOwned>(path: &str) -> Result<T> {
    let raw = test_fs::read_to_string(path).with_context(|| format!("read {path}"))?;
    toml::from_str(&raw).with_context(|| format!("parse {path}"))
}

/// Render the staging directory name for a target.
fn staging_dir(common: &StagingCommon, target: &StagingTarget) -> String {
    common
        .staging_dir_template
        .replace("{bin_name}", &common.bin_name)
        .replace("{platform}", &target.platform)
        .replace("{arch}", &target.arch)
}

/// Build the release asset name the upload action derives for a binary.
fn expected_asset_name(artefact: &str, common: &StagingCommon, target: &StagingTarget) -> String {
    let staged = staging_dir(common, target);
    let bin_name = &common.bin_name;
    let bin_ext = &target.bin_ext;
    format!("{artefact}__{staged}-{bin_name}{bin_ext}")
}

#[test]
fn binstall_overrides_match_the_release_assets() -> Result<()> {
    let manifest: CargoManifest = load(CARGO_MANIFEST)?;
    let staging: StagingConfig = load(STAGING_CONFIG)?;
    let workflow = test_fs::read_to_string(RELEASE_WORKFLOW)
        .with_context(|| format!("read {RELEASE_WORKFLOW}"))?;
    let binstall = &manifest.package.metadata.binstall;

    ensure!(
        binstall.pkg_fmt == "bin",
        "release assets are unarchived binaries; pkg-fmt should be `bin`, found `{}`",
        binstall.pkg_fmt
    );
    ensure!(
        binstall.overrides.len() == WORKFLOW_ARTEFACT_NAMES.len(),
        "every released target needs a binstall override; found {}",
        binstall.overrides.len()
    );

    for (staging_key, artefact) in WORKFLOW_ARTEFACT_NAMES {
        ensure!(
            workflow.contains(&format!("target_key: {staging_key}")),
            "{RELEASE_WORKFLOW} should build staging target {staging_key}"
        );
        let target = staging
            .targets
            .get(staging_key)
            .with_context(|| format!("{STAGING_CONFIG} should define target {staging_key}"))?;
        let entry = binstall
            .overrides
            .get(&target.target)
            .with_context(|| format!("binstall override missing for {}", target.target))?;
        let expected = format!(
            "{{ repo }}/releases/download/v{{ version }}/{asset}",
            asset = expected_asset_name(artefact, &staging.common, target)
        );
        ensure!(
            entry.pkg_url == expected,
            "binstall override for {triple} should resolve the released asset\n  expected: {expected}\n  found:    {found}",
            triple = target.target,
            found = entry.pkg_url
        );
    }
    Ok(())
}

#[test]
fn binstall_overrides_never_reference_the_package_name() -> Result<()> {
    let manifest: CargoManifest = load(CARGO_MANIFEST)?;
    let package_name = &manifest.package.name;
    ensure!(
        package_name == "netsuke-build",
        "Cargo package name drifted: {package_name}"
    );
    for (triple, entry) in &manifest.package.metadata.binstall.overrides {
        ensure!(
            !entry.pkg_url.contains(package_name),
            "binstall override for {triple} names the package rather than the binary"
        );
        ensure!(
            !entry.pkg_url.contains("{ name }"),
            "binstall override for {triple} interpolates the package name"
        );
    }
    Ok(())
}
