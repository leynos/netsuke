//! Validate release staging configuration for generated help artefacts.

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use std::{fs, path::PathBuf};
use toml::Value;

fn release_staging_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".github")
        .join("release-staging.toml")
}

fn release_staging_contents() -> Result<String> {
    let path = release_staging_path();
    fs::read_to_string(&path)
        .with_context(|| format!("read release staging config from {}", path.display()))
}

fn goreleaser_contents() -> Result<String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".goreleaser.yaml");
    fs::read_to_string(&path)
        .with_context(|| format!("read GoReleaser config from {}", path.display()))
}

fn staging_config() -> Result<Value> {
    release_staging_contents()?
        .parse::<Value>()
        .context("parse release staging TOML")
}

fn artefact_sources(config: &Value) -> Vec<&str> {
    let mut sources = Vec::new();
    if let Some(common) = config
        .get("common")
        .and_then(|common| common.get("artefacts"))
        .and_then(Value::as_array)
    {
        sources.extend(common.iter().filter_map(artefact_source));
    }
    if let Some(targets) = config.get("targets").and_then(Value::as_table) {
        for target in targets.values() {
            if let Some(artefacts) = target.get("artefacts").and_then(Value::as_array) {
                sources.extend(artefacts.iter().filter_map(artefact_source));
            }
        }
    }
    sources
}

fn artefact_source(artefact: &Value) -> Option<&str> {
    artefact.get("source").and_then(Value::as_str)
}

#[test]
fn release_staging_config_exists() {
    assert!(
        release_staging_path().is_file(),
        ".github/release-staging.toml should exist"
    );
}

#[rstest]
#[case("target/orthohelp")]
#[case("target/orthohelp/{target}/release/man/man1/{bin_name}.1")]
#[case("target/orthohelp/{target}/release/powershell/Netsuke/Netsuke.psm1")]
#[case("target/orthohelp/{target}/release/powershell/Netsuke/Netsuke.psd1")]
#[case("target/orthohelp/{target}/release/powershell/Netsuke/en-US/Netsuke-help.xml")]
#[case("target/orthohelp/{target}/release/powershell/Netsuke/en-US/about_Netsuke.help.txt")]
fn release_staging_declares_orthohelp_sources(#[case] expected: &str) -> Result<()> {
    let config = staging_config()?;
    let sources = artefact_sources(&config);
    ensure!(
        sources.iter().any(|source| source.contains(expected)),
        "expected release staging source {expected}, got {sources:?}"
    );
    Ok(())
}

#[rstest]
#[case("target/generated-man")]
#[case("OUT_DIR")]
#[case("clap_mangen")]
fn release_staging_omits_build_script_help_sources(#[case] removed: &str) -> Result<()> {
    let contents = release_staging_contents()?;
    ensure!(
        !contents.contains(removed),
        "release staging config should not reference {removed}"
    );
    Ok(())
}

#[rstest]
#[case("x86_64-unknown-linux-gnu")]
#[case("aarch64-unknown-linux-gnu")]
#[case("x86_64-apple-darwin")]
#[case("aarch64-apple-darwin")]
#[case("x86_64-unknown-freebsd")]
fn goreleaser_manpage_fallback_uses_rust_target_triples(#[case] target: &str) -> Result<()> {
    let contents = goreleaser_contents()?;
    ensure!(
        contents.contains(target),
        "GoReleaser manpage fallback should include Rust target triple {target}"
    );
    ensure!(
        contents.contains("target/orthohelp/${RUST_TARGET}/release/man/man1/netsuke.1"),
        "GoReleaser manpage fallback should resolve orthohelp output by Rust target"
    );
    ensure!(
        !contents.contains("target/orthohelp/${GOOS}-${GOARCH}"),
        "GoReleaser manpage fallback must not use Go OS/architecture paths"
    );
    Ok(())
}
