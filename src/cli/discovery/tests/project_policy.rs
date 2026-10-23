//! Unit tests for project-chain fetch-policy provenance and quarantine.

use super::*;
use crate::host_pattern::HostPattern;
use anyhow::{Context, ensure};
use serde_json::json;

/// Create an empty file layer with a stable path for scoping assertions.
fn empty_layer(index: usize) -> MergeLayer<'static> {
    MergeLayer::file(
        Cow::Owned(json!({})),
        Some(format!("layer-{index}.toml").into()),
    )
}

/// Report whether `fields` are absent from one configuration value.
fn value_has_none_of(value: &serde_json::Value, fields: &[&str]) -> bool {
    fields.iter().all(|field| value.get(*field).is_none())
}

/// Check the accumulated request's order and last-present scalar semantics.
///
/// # Errors
///
/// Returns an error when any captured grant or default-deny value is out of
/// dependency-first order.
fn check_dependency_first_request(request: &ProjectFetchPolicyRequest) -> anyhow::Result<()> {
    ensure!(
        request.default_deny == Some(false),
        "last present project default should win"
    );
    ensure!(
        request.allow_scheme
            == [
                "base-scheme".to_owned(),
                "middle-scheme".to_owned(),
                "primary-scheme".to_owned(),
            ],
        "schemes should append in dependency-first order"
    );
    ensure!(
        request.allow_host
            == [
                HostPattern::parse("base.example")?,
                HostPattern::parse("middle.example")?,
                HostPattern::parse("primary.example")?,
            ],
        "hosts should append in dependency-first order"
    );
    Ok(())
}

/// Check that project grants are removed while cumulative blocks remain.
///
/// # Errors
///
/// Returns an error when a grant survives quarantine or a block is discarded.
fn check_quarantined_fields(
    base: &serde_json::Value,
    middle: &serde_json::Value,
    primary: &serde_json::Value,
) -> anyhow::Result<()> {
    ensure!(
        value_has_none_of(
            base,
            &[
                "fetch_default_deny",
                "fetch_allow_scheme",
                "fetch_allow_host",
                "env_allow_var",
            ]
        ) && value_has_none_of(
            middle,
            &[
                "fetch_allow_scheme",
                "fetch_allow_host",
                "trust_project_fetch_policy",
            ]
        ) && value_has_none_of(
            primary,
            &[
                "fetch_default_deny",
                "fetch_allow_scheme",
                "fetch_allow_host",
            ]
        ),
        "grant, trust, and environment allow fields should be stripped"
    );
    ensure!(
        base.get("fetch_block_host").is_some()
            && base.get("env_block_var").is_some()
            && middle.get("fetch_block_host").is_some(),
        "block fields should remain cumulative"
    );
    Ok(())
}

#[test]
fn project_fetch_scoping_covers_inherited_layers_through_the_primary_index() {
    let layers =
        scope_project_fetch_layers(vec![empty_layer(0), empty_layer(1), empty_layer(2)], 1);

    assert_eq!(
        layers.iter().map(|layer| layer.scope).collect::<Vec<_>>(),
        [FileScope::Project, FileScope::Project, FileScope::Operator]
    );
    assert!(layers.iter().all(|layer| !layer.project_chain));
}

#[test]
fn automatic_project_chain_keeps_budget_authority_on_every_occurrence() {
    let layers = scope_project_chain(vec![empty_layer(0), empty_layer(1), empty_layer(2)], 1);

    assert_eq!(
        layers.iter().map(|layer| layer.scope).collect::<Vec<_>>(),
        [FileScope::Project, FileScope::Project, FileScope::Operator]
    );
    assert!(layers.iter().all(|layer| layer.project_chain));
}

#[test]
fn malformed_quarantined_policy_fields_remain_configuration_errors() {
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
            .expect_err("malformed quarantined field should fail discovery");
        assert!(matches!(
            error.as_ref(),
            ortho_config::OrthoError::Validation { key, .. } if key == field
        ));
        assert!(
            value.get(field).is_some(),
            "invalid {field} must not be stripped before validation"
        );
    }
}

#[test]
fn project_chain_requests_append_grants_and_use_the_last_present_default() -> anyhow::Result<()> {
    let base = json!({
        "fetch_default_deny": true,
        "fetch_allow_scheme": ["base-scheme"],
        "fetch_allow_host": ["base.example"],
        "fetch_block_host": ["base-block.example"],
        "env_allow_var": ["BASE_SECRET"],
        "env_block_var": ["BASE_BLOCKED"],
    });
    let middle = json!({
        "fetch_allow_scheme": ["middle-scheme"],
        "fetch_allow_host": ["middle.example"],
        "fetch_block_host": ["middle-block.example"],
        "trust_project_fetch_policy": true,
    });
    let primary = json!({
        "fetch_default_deny": false,
        "fetch_allow_scheme": ["primary-scheme"],
        "fetch_allow_host": ["primary.example"],
    });

    let scoped = scope_project_fetch_layers(
        [base, middle, primary]
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                MergeLayer::file(
                    Cow::Owned(value),
                    Some(format!("layer-{index}.toml").into()),
                )
            })
            .collect(),
        2,
    );
    let resolved = retain_layers_and_resolve_json(scoped);
    check_dependency_first_request(
        resolved
            .project_request
            .as_ref()
            .context("project chain should produce a request")?,
    )?;
    let mut layers = resolved.layers.into_iter();
    let base_layer = layers
        .next()
        .context("base layer should remain")?
        .into_value();
    let middle_layer = layers
        .next()
        .context("middle layer should remain")?
        .into_value();
    let primary_layer = layers
        .next()
        .context("primary layer should remain")?
        .into_value();
    check_quarantined_fields(&base_layer, &middle_layer, &primary_layer)?;
    ensure!(
        resolved.errors.is_empty(),
        "valid requests should not add errors"
    );
    Ok(())
}

#[test]
fn empty_project_layer_is_present_but_an_absent_chain_is_not() {
    let mut resolved = ResolvedFileLayers::default();
    assert!(resolved.project_request.is_none());

    let mut empty = json!({});
    quarantine_fetch_policy(&mut empty, &mut resolved);

    assert_eq!(
        resolved
            .project_request
            .as_ref()
            .map(|request| request.default_deny),
        Some(None)
    );
}

#[test]
fn malformed_project_chain_requests_accumulate_validation_errors() {
    let mut resolved = ResolvedFileLayers::default();
    let mut inherited = json!({"fetch_allow_scheme": "http"});
    let mut primary = json!({"trust_project_fetch_policy": "true"});

    quarantine_fetch_policy(&mut inherited, &mut resolved);
    quarantine_fetch_policy(&mut primary, &mut resolved);

    assert_eq!(resolved.errors.len(), 2);
    assert!(has_validation_error(&resolved, 0, "fetch_allow_scheme"));
    assert!(has_validation_error(
        &resolved,
        1,
        "trust_project_fetch_policy"
    ));
    assert!(inherited.get("fetch_allow_scheme").is_some());
    assert!(primary.get("trust_project_fetch_policy").is_some());
}

/// Check whether one accumulated error preserves the expected validation key.
fn has_validation_error(resolved: &ResolvedFileLayers, index: usize, key: &str) -> bool {
    resolved.errors.get(index).is_some_and(|error| {
        matches!(
            error.as_ref(),
            OrthoError::Validation { key: found, .. } if found == key
        )
    })
}
