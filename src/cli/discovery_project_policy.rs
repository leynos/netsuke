//! Quarantine parsing for fetch-policy fields from the primary project file.
//!
//! Validates each untrusted request before the generic merge removes it from
//! the project layer, preserving configuration errors rather than treating
//! malformed policy values as absent values.

use ortho_config::OrthoResult;
use serde::de::DeserializeOwned;

use super::super::validation::validation_error;
use super::ProjectFetchPolicyRequest;

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
