//! Tests for occurrence-level project policy quarantine.

use super::super::paths::FsPathNormalizer;
use super::*;
use crate::host_pattern::HostPattern;
use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use serde_json::{Value, json};
use std::path::Path;

/// Build a file layer with an optional path for discovery-scope tests.
fn file_layer(value: Value, path: Option<Utf8PathBuf>) -> MergeLayer<'static> {
    MergeLayer::file(Cow::Owned(value), path)
}

/// Convert a test path to the UTF-8 path type used by configuration layers.
fn utf8_path(path: &Path) -> Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(path.to_path_buf()).map_err(|non_utf8_path| {
        anyhow::anyhow!("test path is not UTF-8: {}", non_utf8_path.display())
    })
}

/// Quarantine grants across an automatic project chain while retaining
/// cumulative blocks and primary-only fetch extraction.
#[test]
fn project_chain_quarantines_ancestor_grants_and_keeps_fetch_fields() {
    let scoped = scope_project_chain(
        vec![
            file_layer(
                json!({
                    "env_allow_var": ["CI_TOKEN"],
                    "env_block_var": ["BLOCKED_TOKEN"],
                    "fetch_allow_host": ["ancestor.example"],
                    "trust_project_fetch_policy": true
                }),
                None,
            ),
            file_layer(
                json!({
                    "env_allow_var": ["PRIMARY_TOKEN"],
                    "fetch_allow_host": ["primary.example"],
                    "trust_project_fetch_policy": true
                }),
                None,
            ),
        ],
        1,
    );

    assert!(
        scoped.iter().all(|layer| layer.project_chain),
        "each project-rooted occurrence must keep its chain provenance"
    );
    let mut resolved = retain_layers_and_resolve_json(scoped);
    let ancestor = resolved.layers.remove(0).into_value();
    let primary = resolved.layers.remove(0).into_value();

    assert!(ancestor.get("env_allow_var").is_none());
    assert_eq!(
        ancestor.get("env_block_var"),
        Some(&json!(["BLOCKED_TOKEN"]))
    );
    assert_eq!(
        ancestor.get("fetch_allow_host"),
        Some(&json!(["ancestor.example"]))
    );
    assert_eq!(
        ancestor.get("trust_project_fetch_policy"),
        Some(&json!(true))
    );
    assert!(primary.get("env_allow_var").is_none());
    assert!(primary.get("fetch_allow_host").is_none());
    assert_eq!(
        resolved.project_request.map(|request| request.allow_host),
        Some(vec![
            HostPattern::parse("primary.example").expect("valid host pattern")
        ])
    );
}

/// Keep an environment grant when the occurrence came from an operator root.
#[test]
fn operator_occurrence_retains_environment_grants() {
    let occurrence =
        ScopedFileLayer::operator(file_layer(json!({ "env_allow_var": ["CI_TOKEN"] }), None));
    assert!(!occurrence.project_chain);

    let resolved = retain_layers_and_resolve_json(vec![occurrence]);
    let layer = resolved.layers.into_iter().next().expect("operator layer");
    assert_eq!(
        layer.into_value().get("env_allow_var"),
        Some(&json!(["CI_TOKEN"]))
    );
}

/// Quarantine an explicitly selected project primary while leaving its
/// explicit ancestors at operator authority.
#[test]
fn explicit_primary_is_quarantined_but_explicit_ancestor_retains_grants() -> Result<()> {
    let project = tempfile::tempdir()?;
    let ancestor_path = utf8_path(&project.path().join("base.toml"))?;
    let primary_path = utf8_path(&project.path().join(".netsuke.toml"))?;
    let layers = vec![
        file_layer(
            json!({ "env_allow_var": ["ANCESTOR_TOKEN"] }),
            Some(ancestor_path),
        ),
        file_layer(
            json!({ "env_allow_var": ["PRIMARY_TOKEN"] }),
            Some(primary_path),
        ),
    ];

    let scoped = scope_selected_primary_layer(layers, Some(project.path()), &FsPathNormalizer);
    ensure!(
        scoped.len() == 2,
        "both explicit occurrences should be retained"
    );
    ensure!(
        scoped.iter().all(|layer| !layer.project_chain),
        "explicitly selected ancestors keep operator provenance"
    );
    let resolved = retain_layers_and_resolve_json(scoped);

    let mut resolved_layers = resolved.layers.into_iter();
    let ancestor = resolved_layers
        .next()
        .context("ancestor layer should remain available")?
        .into_value();
    let primary = resolved_layers
        .next()
        .context("primary layer should remain available")?
        .into_value();
    ensure!(
        ancestor.get("env_allow_var") == Some(&json!(["ANCESTOR_TOKEN"])),
        "an explicit ancestor keeps its environment grant"
    );
    ensure!(
        primary.get("env_allow_var").is_none(),
        "the selected project primary loses its environment grant"
    );
    Ok(())
}

/// Preserve the field and return its typed validation error when a project
/// occurrence supplies a malformed grant.
#[test]
fn malformed_project_chain_grant_remains_a_validation_error() {
    let resolved = retain_layers_and_resolve_json(scope_project_chain(
        vec![file_layer(json!({ "env_allow_var": "CI_TOKEN" }), None)],
        0,
    ));

    assert!(matches!(
        resolved.errors.first().map(AsRef::as_ref),
        Some(OrthoError::Validation { key, .. }) if key == "env_allow_var"
    ));
    let layer = resolved.layers.into_iter().next().expect("malformed layer");
    assert_eq!(
        layer.into_value().get("env_allow_var"),
        Some(&json!("CI_TOKEN"))
    );
}

/// Keep malformed fetch fields within the fetch quarantine validator.
#[test]
fn malformed_primary_fetch_fields_remain_configuration_errors() {
    let cases = [
        (
            "fetch_default_deny",
            json!({ "fetch_default_deny": "true" }),
        ),
        (
            "fetch_allow_scheme",
            json!({ "fetch_allow_scheme": "https" }),
        ),
        (
            "fetch_allow_host",
            json!({ "fetch_allow_host": "example.org" }),
        ),
        (
            "trust_project_fetch_policy",
            json!({ "trust_project_fetch_policy": "true" }),
        ),
    ];

    for (field, mut value) in cases {
        let error = take_project_fetch_policy_request(&mut value)
            .expect_err("malformed fetch-policy field should fail discovery");
        assert!(matches!(
            error.as_ref(),
            OrthoError::Validation { key, .. } if key == field
        ));
        assert!(
            value.get(field).is_some(),
            "invalid {field} must not be stripped before validation"
        );
    }
}
