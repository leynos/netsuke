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

/// Quarantine grants across a project chain while retaining cumulative blocks.
#[test]
fn project_chain_quarantines_ancestor_fetch_grants_and_keeps_blocks() {
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
    assert!(ancestor.get("fetch_allow_host").is_none());
    assert!(ancestor.get("trust_project_fetch_policy").is_none());
    assert!(primary.get("env_allow_var").is_none());
    assert!(primary.get("fetch_allow_host").is_none());
    assert_eq!(
        resolved.project_request.map(|request| request.allow_host),
        Some(vec![
            HostPattern::parse("ancestor.example").expect("valid host pattern"),
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

/// Quarantine an explicitly selected project primary and its ancestors.
#[test]
fn explicit_project_chain_quarantines_primary_and_ancestor_grants() -> Result<()> {
    let project = tempfile::tempdir()?;
    let ancestor_path = utf8_path(&project.path().join("base.toml"))?;
    let primary_path = utf8_path(&project.path().join(".netsuke.toml"))?;
    let layers = vec![
        file_layer(
            json!({
                "env_allow_var": ["ANCESTOR_TOKEN"],
                "fetch_allow_host": ["ancestor.example"]
            }),
            Some(ancestor_path),
        ),
        file_layer(
            json!({
                "env_allow_var": ["PRIMARY_TOKEN"],
                "fetch_allow_host": ["primary.example"]
            }),
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
    ensure!(ancestor.get("env_allow_var").is_none());
    ensure!(ancestor.get("fetch_allow_host").is_none());
    ensure!(
        primary.get("env_allow_var").is_none(),
        "the selected project primary loses its environment grant"
    );
    ensure!(primary.get("fetch_allow_host").is_none());
    ensure!(
        resolved.project_request.map(|request| request.allow_host)
            == Some(vec![
                HostPattern::parse("ancestor.example")?,
                HostPattern::parse("primary.example")?
            ]),
        "explicit project-chain grants are retained in dependency order"
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

/// Reject every reset representation for the project blocklists while leaving
/// the supplied value untouched, so no later merge can read it as a clear.
#[test]
fn malformed_project_blocklists_remain_validation_errors() {
    let malformed_values = [
        ("null", json!(null)),
        ("a scalar number", json!(1)),
        ("a scalar string", json!("BLOCKED")),
        ("an object", json!({"blocked": "TOKEN"})),
        ("an array containing null", json!([null])),
        ("an array containing a number", json!([1])),
    ];

    for (description, supplied) in malformed_values {
        for field in ["env_block_var", "fetch_block_host"] {
            let resolved = retain_layers_and_resolve_json(scope_project_chain(
                vec![file_layer(json!({ (field): supplied.clone() }), None)],
                0,
            ));

            assert!(
                matches!(
                    resolved.errors.first().map(AsRef::as_ref),
                    Some(OrthoError::Validation { key, .. }) if key == field
                ),
                "{description} in {field} must produce its own validation error"
            );
            let retained = resolved
                .layers
                .into_iter()
                .next()
                .expect("malformed layer is retained")
                .into_value();
            assert_eq!(
                retained.get(field),
                Some(&supplied),
                "{description} in {field} must not be stripped or rewritten"
            );
        }
    }
}

/// Reject an invalid host pattern inside a project fetch blocklist.
#[test]
fn invalid_project_fetch_block_host_pattern_is_a_validation_error() {
    let supplied = json!([""]);
    let resolved = retain_layers_and_resolve_json(scope_project_chain(
        vec![file_layer(
            json!({ "fetch_block_host": supplied.clone() }),
            None,
        )],
        0,
    ));

    assert!(matches!(
        resolved.errors.first().map(AsRef::as_ref),
        Some(OrthoError::Validation { key, .. }) if key == "fetch_block_host"
    ));
    let retained = resolved
        .layers
        .into_iter()
        .next()
        .expect("malformed layer is retained")
        .into_value();
    assert_eq!(retained.get("fetch_block_host"), Some(&supplied));
}

/// Preserve valid and empty project blocklists as cumulative restrictions.
#[test]
fn valid_project_blocklists_are_preserved_without_errors() {
    let resolved = retain_layers_and_resolve_json(scope_project_chain(
        vec![file_layer(
            json!({
                "env_block_var": ["BLOCKED_TOKEN"],
                "fetch_block_host": ["blocked.example", "*.wild.example"],
            }),
            None,
        )],
        0,
    ));

    assert!(resolved.errors.is_empty());
    let value = resolved
        .layers
        .into_iter()
        .next()
        .expect("valid layer is retained")
        .into_value();
    assert_eq!(value.get("env_block_var"), Some(&json!(["BLOCKED_TOKEN"])));
    assert_eq!(
        value.get("fetch_block_host"),
        Some(&json!(["blocked.example", "*.wild.example"]))
    );

    let empty = retain_layers_and_resolve_json(scope_project_chain(
        vec![file_layer(
            json!({ "env_block_var": [], "fetch_block_host": [] }),
            None,
        )],
        0,
    ));
    assert!(empty.errors.is_empty());
    let empty_value = empty
        .layers
        .into_iter()
        .next()
        .expect("empty layer is retained")
        .into_value();
    assert_eq!(empty_value.get("env_block_var"), Some(&json!([])));
    assert_eq!(empty_value.get("fetch_block_host"), Some(&json!([])));
}

/// Validate both blocklists for an explicitly selected project occurrence,
/// which carries project scope without automatic-chain provenance.
#[test]
fn explicit_project_occurrence_validates_both_blocklists() {
    let resolved = retain_layers_and_resolve_json(scope_project_fetch_layers(
        vec![file_layer(
            json!({"env_block_var": null, "fetch_block_host": null}),
            None,
        )],
        0,
    ));

    assert_eq!(resolved.errors.len(), 2);
    assert!(matches!(
        resolved.errors.first().map(AsRef::as_ref),
        Some(OrthoError::Validation { key, .. }) if key == "env_block_var"
    ));
    assert!(matches!(
        resolved.errors.get(1).map(AsRef::as_ref),
        Some(OrthoError::Validation { key, .. }) if key == "fetch_block_host"
    ));
}

/// Validate the environment blocklist for a chain occurrence that the fetch
/// quarantine treats as operator-authority, because the chain governs
/// environment policy while fetch policy follows the selected primary index.
#[test]
fn chain_environment_blocklist_is_validated_for_operator_scoped_occurrence() {
    let resolved = retain_layers_and_resolve_json(scope_project_chain(
        vec![
            file_layer(json!({"fetch_block_host": ["blocked.example"]}), None),
            file_layer(
                json!({"env_block_var": null, "fetch_block_host": null}),
                None,
            ),
        ],
        0,
    ));

    assert_eq!(
        resolved.errors.len(),
        1,
        "only the chain-governed environment blocklist is validated here"
    );
    assert!(matches!(
        resolved.errors.first().map(AsRef::as_ref),
        Some(OrthoError::Validation { key, .. }) if key == "env_block_var"
    ));
}

/// Leave operator-authority occurrences with their existing semantics: a
/// malformed blocklist is not reclassified as a project-boundary failure.
#[test]
fn operator_occurrence_retains_malformed_blocklists_without_new_errors() {
    let supplied = json!({"env_block_var": null, "fetch_block_host": null});
    let resolved = retain_layers_and_resolve_json(vec![ScopedFileLayer::operator(file_layer(
        supplied.clone(),
        None,
    ))]);

    assert!(resolved.errors.is_empty());
    let retained = resolved
        .layers
        .into_iter()
        .next()
        .expect("operator layer is retained")
        .into_value();
    assert_eq!(retained, supplied);
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
