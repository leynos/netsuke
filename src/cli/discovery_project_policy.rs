//! Preserve project-chain provenance and quarantine fetch-policy and budget fields.
//!
//! Validates each untrusted request before the generic merge removes it from
//! the project layer, preserving configuration errors rather than treating
//! malformed policy values as absent values. Fetch grants are primary-scoped;
//! budget restrictions apply throughout an automatically discovered project chain.

use ortho_config::{MergeLayer, OrthoError, OrthoResult};
use serde::de::DeserializeOwned;
use std::borrow::Cow;
use std::path::Path;
use std::sync::Arc;

use super::super::validation::validation_error;
use super::json::json_from_value;
use super::paths::{PathNormalizer, comparison_key, project_scope_file};
use super::{ProjectFetchPolicyRequest, ProjectManifestBudgetRequest};

/// Identify whether a loaded file is the primary project configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FileScope {
    /// A layer governed by ordinary configuration precedence.
    Operator,
    /// The primary project configuration file.
    Project,
}

/// Keep a loaded file's authority until quarantine has completed.
pub(super) struct ScopedFileLayer {
    /// The original, unmerged file layer.
    pub(super) layer: MergeLayer<'static>,
    /// Authority inherited from the root that loaded this occurrence.
    scope: FileScope,
    /// Whether this occurrence belongs to the automatically discovered project chain.
    project_budget: bool,
}

impl ScopedFileLayer {
    /// Retain a file occurrence loaded through an operator root.
    pub(super) const fn operator(layer: MergeLayer<'static>) -> Self {
        Self {
            layer,
            scope: FileScope::Operator,
            project_budget: false,
        }
    }
}

/// Mark only the selected primary configuration layer as project-scoped.
pub(super) fn scope_primary_project_layer(
    layers: Vec<MergeLayer<'static>>,
    project_index: usize,
) -> Vec<ScopedFileLayer> {
    layers
        .into_iter()
        .enumerate()
        .map(|(index, layer)| ScopedFileLayer {
            layer,
            scope: if index == project_index {
                FileScope::Project
            } else {
                FileScope::Operator
            },
            project_budget: false,
        })
        .collect()
}

/// Retain budget authority across one automatically discovered project chain.
///
/// Chain boundaries are available when discovery returns its first root or
/// appends the project root. Fetch-policy quarantine remains primary-only.
pub(super) fn scope_project_chain(
    layers: Vec<MergeLayer<'static>>,
    project_index: usize,
) -> Vec<ScopedFileLayer> {
    scope_primary_project_layer(layers, project_index)
        .into_iter()
        .map(|mut layer| {
            layer.project_budget = true;
            layer
        })
        .collect()
}

/// Identify the primary project file before discarding chain boundaries.
pub(super) fn scope_selected_primary_layer(
    layers: Vec<MergeLayer<'static>>,
    directory: Option<&Path>,
    normalizer: &impl PathNormalizer,
) -> Vec<ScopedFileLayer> {
    let project_key = project_scope_file(directory)
        .map(|path| comparison_key(normalizer, &path.to_string_lossy()));
    let project_index = layers.iter().position(|layer| {
        layer
            .path()
            .zip(project_key.as_deref())
            .is_some_and(|(path, expected)| path.as_str() == expected.to_string_lossy())
    });
    if let Some(index) = project_index {
        scope_primary_project_layer(layers, index)
    } else {
        layers.into_iter().map(ScopedFileLayer::operator).collect()
    }
}

/// Retain JSON preference independently of deferred policy-validation failures.
#[derive(Default)]
pub(super) struct ResolvedFileLayers {
    /// Owned file values, including unstripped malformed requests on failure.
    pub(super) layers: Vec<MergeLayer<'static>>,
    /// Last valid file preference, including layers after an invalid request.
    pub(super) json_preference: bool,
    /// Quarantined request from the primary project layer, when present.
    pub(super) project_request: Option<ProjectFetchPolicyRequest>,
    /// Quarantined manifest-budget restrictions from project layers.
    pub(super) project_budget_request: ProjectManifestBudgetRequest,
    /// Original typed errors that prevent the generic merge from succeeding.
    pub(super) errors: Vec<Arc<OrthoError>>,
}

/// Quarantine project occurrences while preserving the complete JSON preference.
///
/// Keep errors beside owned layers instead of returning early: startup must
/// still honour JSON preferences in the failing file and later file layers.
pub(super) fn retain_layers_and_resolve_json(layers: Vec<ScopedFileLayer>) -> ResolvedFileLayers {
    let mut resolved = ResolvedFileLayers {
        json_preference: super::Cli::default().json,
        ..ResolvedFileLayers::default()
    };
    for ScopedFileLayer {
        layer,
        scope,
        project_budget,
    } in layers
    {
        let path = layer.path().map(ToOwned::to_owned);
        let mut value = layer.into_value();
        if let Some(json) = json_from_value(&value) {
            resolved.json_preference = json;
        }
        if project_budget {
            quarantine_manifest_budget(&mut value, &mut resolved);
        }
        if scope == FileScope::Project {
            quarantine_fetch_policy(&mut value, &mut resolved);
        }
        resolved
            .layers
            .push(MergeLayer::file(Cow::Owned(value), path));
    }
    resolved
}

/// Retain a project-chain budget request or its deferred validation error.
///
/// Discovery calls this only for project-budget occurrences so generic merging
/// cannot widen operator ceilings, while later JSON preferences remain readable.
fn quarantine_manifest_budget(value: &mut serde_json::Value, resolved: &mut ResolvedFileLayers) {
    match take_project_manifest_budget_request(value) {
        Ok(request) => resolved.project_budget_request.narrow_with(&request),
        Err(error) => resolved.errors.push(error),
    }
}

/// Retain a primary-project fetch request or its deferred validation error.
///
/// Keep this primary-only operation separate from whole-chain budget quarantine
/// to preserve their different authority boundaries during discovery.
fn quarantine_fetch_policy(value: &mut serde_json::Value, resolved: &mut ResolvedFileLayers) {
    match take_project_fetch_policy_request(value) {
        Ok(request) => resolved.project_request = Some(request),
        Err(error) => resolved.errors.push(error),
    }
}

/// Validate and quarantine manifest limits before generic configuration merging.
fn take_project_manifest_budget_request(
    value: &mut serde_json::Value,
) -> OrthoResult<ProjectManifestBudgetRequest> {
    let Some(fields) = value.as_object_mut() else {
        return Ok(ProjectManifestBudgetRequest::default());
    };
    let request = ProjectManifestBudgetRequest {
        evaluation_fuel: parse_project_policy_field(fields, "manifest_evaluation_fuel")?,
        manifest_fuel: parse_project_policy_field(fields, "manifest_fuel")?,
        rendered_value_bytes: parse_project_policy_field(fields, "manifest_rendered_value_bytes")?,
        rendered_manifest_bytes: parse_project_policy_field(
            fields,
            "manifest_rendered_manifest_bytes",
        )?,
        source_bytes: parse_project_policy_field(fields, "manifest_source_bytes")?,
        foreach_cardinality: parse_project_policy_field(fields, "manifest_foreach_cardinality")?,
        expanded_entries: parse_project_policy_field(fields, "manifest_expanded_entries")?,
    };
    for field in [
        "manifest_evaluation_fuel",
        "manifest_fuel",
        "manifest_rendered_value_bytes",
        "manifest_rendered_manifest_bytes",
        "manifest_source_bytes",
        "manifest_foreach_cardinality",
        "manifest_expanded_entries",
    ] {
        if fields
            .get(field)
            .is_some_and(|candidate| candidate.as_u64() == Some(0))
        {
            return Err(validation_error(field, "must be greater than zero"));
        }
    }
    for field in [
        "manifest_evaluation_fuel",
        "manifest_fuel",
        "manifest_rendered_value_bytes",
        "manifest_rendered_manifest_bytes",
        "manifest_source_bytes",
        "manifest_foreach_cardinality",
        "manifest_expanded_entries",
    ] {
        fields.remove(field);
    }
    Ok(request)
}

/// Capture and remove project fetch-policy grants from one JSON layer.
///
/// Project configuration may request a narrower policy, but generic precedence
/// and append merging must not grant it authority to widen operator policy.
pub(super) fn take_project_fetch_policy_request(
    value: &mut serde_json::Value,
) -> OrthoResult<ProjectFetchPolicyRequest> {
    let Some(fields) = value.as_object_mut() else {
        return Ok(ProjectFetchPolicyRequest::default());
    };
    let default_deny = parse_project_policy_field(fields, "fetch_default_deny")?;
    let allow_scheme =
        parse_project_policy_field(fields, "fetch_allow_scheme")?.unwrap_or_default();
    let allow_host = parse_project_policy_field(fields, "fetch_allow_host")?.unwrap_or_default();
    let _: Option<bool> = parse_project_policy_field(fields, "trust_project_fetch_policy")?;
    fields.remove("fetch_default_deny");
    fields.remove("fetch_allow_scheme");
    fields.remove("fetch_allow_host");
    fields.remove("trust_project_fetch_policy");
    Ok(ProjectFetchPolicyRequest {
        default_deny,
        allow_scheme,
        allow_host,
    })
}

/// Deserialize one quarantined project-policy field without stripping it first.
///
/// Retaining the JSON value until every fetch-policy field validates prevents a
/// malformed untrusted request from being mistaken for an absent request.
fn parse_project_policy_field<T>(
    fields: &serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> OrthoResult<Option<T>>
where
    T: DeserializeOwned,
{
    fields
        .get(field)
        .cloned()
        .map(|value| {
            serde_json::from_value(value)
                .map_err(|error| validation_error(field, &error.to_string()))
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    //! Unit tests for quarantined project fetch-policy extraction.

    use super::*;
    use serde_json::json;

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
}
