//! Contract tests for the `cargo binstall` metadata in `Cargo.toml`.
//!
//! The README and the users' guide advertise `cargo binstall netsuke-build`.
//! Release assets for binstall are `{package_name}-{version}-{target}.tar.gz`
//! archives staged by `stage-release-artefacts` (`[common.binstall]` in
//! `.github/release-staging.toml`) and hoisted to the release root by
//! `.github/workflows/release.yml`, so a single `pkg-url` template covers
//! every released target. These tests hold the metadata, the staging
//! configuration, and the release workflow to that contract so a packaging
//! change cannot silently strand the documented command on a source build.

use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::collections::BTreeMap;
use test_support::fs as test_fs;

const CARGO_MANIFEST: &str = "Cargo.toml";
const STAGING_CONFIG: &str = ".github/release-staging.toml";
const RELEASE_WORKFLOW: &str = ".github/workflows/release.yml";

/// The asset-name template the staging action uses when `archive_name` is not
/// overridden. The binstall `pkg-url` in `Cargo.toml` must resolve to the
/// same shape once the archives are hoisted to the release root.
const DEFAULT_ARCHIVE_NAME: &str = "{package_name}-{version}-{target}.tar.gz";

#[derive(Deserialize)]
struct StagingConfig {
    common: StagingCommon,
    targets: BTreeMap<String, StagingTarget>,
}

#[derive(Deserialize)]
struct StagingCommon {
    binstall: Option<StagingBinstall>,
}

#[derive(Deserialize)]
struct StagingBinstall {
    #[serde(default)]
    enabled: bool,
    archive_name: Option<String>,
}

#[derive(Deserialize)]
struct StagingTarget {
    target: String,
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
    #[serde(rename = "pkg-url")]
    pkg_url: String,
    #[serde(rename = "bin-dir")]
    bin_dir: String,
    #[serde(rename = "pkg-fmt")]
    pkg_fmt: String,
    #[serde(default)]
    overrides: BTreeMap<String, toml::Value>,
}

fn load<T: serde::de::DeserializeOwned>(path: &str) -> Result<T> {
    let raw = test_fs::read_to_string(path).with_context(|| format!("read {path}"))?;
    toml::from_str(&raw).with_context(|| format!("parse {path}"))
}

#[test]
fn binstall_metadata_matches_the_staged_archives() -> Result<()> {
    let manifest: CargoManifest = load(CARGO_MANIFEST)?;
    let staging: StagingConfig = load(STAGING_CONFIG)?;
    let binstall = &manifest.package.metadata.binstall;

    ensure!(
        manifest.package.name == "netsuke-build",
        "Cargo package name drifted: {}",
        manifest.package.name
    );
    let staging_binstall = staging
        .common
        .binstall
        .as_ref()
        .with_context(|| format!("{STAGING_CONFIG} should define [common.binstall]"))?;
    ensure!(
        staging_binstall.enabled,
        "{STAGING_CONFIG} should enable cargo-binstall archive staging"
    );
    // The pkg-url below assumes the staging action's default archive name;
    // an override would break the resolved asset names silently.
    ensure!(
        staging_binstall.archive_name.is_none()
            || staging_binstall.archive_name.as_deref() == Some(DEFAULT_ARCHIVE_NAME),
        "{STAGING_CONFIG} must keep the default binstall archive name `{DEFAULT_ARCHIVE_NAME}`"
    );

    ensure!(
        binstall.pkg_fmt == "tgz",
        "release archives are tar.gz; pkg-fmt should be `tgz`, found `{}`",
        binstall.pkg_fmt
    );
    ensure!(
        binstall.pkg_url
            == "{ repo }/releases/download/v{ version }/{ name }-{ version }-{ target }.tar.gz",
        "binstall pkg-url should resolve the hoisted archive names, found `{}`",
        binstall.pkg_url
    );
    ensure!(
        binstall.bin_dir == "{ bin }{ binary-ext }",
        "archives place the bare binary at their root; bin-dir drifted: `{}`",
        binstall.bin_dir
    );
    ensure!(
        binstall.overrides.is_empty(),
        "the single pkg-url template covers every target; per-target overrides drifted back in"
    );
    Ok(())
}

#[test]
fn release_workflow_builds_and_hoists_every_staged_target() -> Result<()> {
    let staging: StagingConfig = load(STAGING_CONFIG)?;
    let workflow = test_fs::read_to_string(RELEASE_WORKFLOW)
        .with_context(|| format!("read {RELEASE_WORKFLOW}"))?;

    ensure!(
        !staging.targets.is_empty(),
        "{STAGING_CONFIG} should define release targets"
    );
    for (staging_key, target) in &staging.targets {
        ensure!(
            workflow.contains(&format!("target_key: {staging_key}")),
            "{RELEASE_WORKFLOW} should build staging target {staging_key} ({})",
            target.target
        );
    }
    ensure!(
        workflow.contains("Hoist cargo-binstall archives"),
        "{RELEASE_WORKFLOW} should hoist binstall archives to the release root \
         so the pkg-url template in {CARGO_MANIFEST} resolves them"
    );
    Ok(())
}
