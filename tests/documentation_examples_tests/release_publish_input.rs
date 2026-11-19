//! Semantic contracts for the reusable release publication example.
//!
//! The guide tells a caller to pass `publish: true`. That instruction is only
//! correct while the reusable workflow still declares `publish` as a boolean
//! input defaulting to `false`, so the example is checked against the workflow
//! it addresses rather than against a copy of the same claim.

use super::documentation_examples::{DocumentedExample, documented_example};
use anyhow::{Context, Result, ensure};
use rstest::rstest;
use serde_yaml::Value;

const EXAMPLE_ID: &str = "guide-release-publish-input";
const INPUT_NAME: &str = "publish";
const RELEASE_WORKFLOW: &str = include_str!("../../.github/workflows/release.yml");

/// Load the marked fragment and the reusable workflow that defines its input.
///
/// # Errors
///
/// Returns an error when either YAML document is malformed or the marked
/// example cannot be loaded.
fn release_example_and_workflow() -> Result<(DocumentedExample, Value, Value)> {
    let example = documented_example(EXAMPLE_ID)?;
    let fragment: Value =
        serde_yaml::from_str(&example.body).context("parse documented workflow fragment")?;
    let workflow: Value =
        serde_yaml::from_str(RELEASE_WORKFLOW).context("parse reusable release workflow")?;
    Ok((example, fragment, workflow))
}

/// Return a string-keyed value from a YAML mapping.
fn mapping_value<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.as_mapping()?.get(Value::String(key.to_owned()))
}

/// Return a mutable string-keyed value from a YAML mapping.
fn mapping_value_mut<'a>(value: &'a mut Value, key: &str) -> Option<&'a mut Value> {
    value
        .as_mapping_mut()?
        .get_mut(Value::String(key.to_owned()))
}

/// Locate the reusable workflow's declaration of the `publish` input.
fn workflow_publish_input(workflow: &Value) -> Result<&Value> {
    mapping_value(workflow, "on")
        .and_then(|on| mapping_value(on, "workflow_call"))
        .and_then(|call| mapping_value(call, "inputs"))
        .and_then(|inputs| mapping_value(inputs, INPUT_NAME))
        .context("release workflow should declare the reusable `publish` input")
}

/// Validate that the documented fragment authorizes publication as described.
///
/// # Errors
///
/// Returns an error when the example is not YAML, when the fragment does not
/// set `with.publish` to the boolean `true`, or when the workflow no longer
/// declares a boolean `publish` input defaulting to `false`.
fn validate_publish_example(
    example: &DocumentedExample,
    fragment: &Value,
    workflow: &Value,
) -> Result<()> {
    ensure!(
        example.language == "yaml",
        "release publication example should use a YAML fence, got {:?}",
        example.language
    );

    let documented = mapping_value(fragment, "with")
        .and_then(Value::as_mapping)
        .context("workflow fragment should contain a `with` mapping")?
        .get(Value::String(INPUT_NAME.to_owned()))
        .context("`with.publish` should be present")?;
    let publishes = documented
        .as_bool()
        .context("`with.publish` should be a YAML boolean, not a quoted string")?;
    ensure!(
        publishes,
        "`with.publish` should be `true` to authorize a release write"
    );

    let workflow_input = workflow_publish_input(workflow)?;
    let input_type = mapping_value(workflow_input, "type")
        .and_then(Value::as_str)
        .context("release workflow input should declare a type")?;
    ensure!(
        input_type == "boolean",
        "release workflow input type should be `boolean`, got {input_type:?}"
    );
    let default = mapping_value(workflow_input, "default")
        .and_then(Value::as_bool)
        .context("release workflow input should have a boolean default")?;
    ensure!(
        !default,
        "the reusable `publish` input should default to `false`, so a silent \
         caller cannot publish"
    );
    Ok(())
}

/// Identify one invalid edit to the parsed documentation fragment or workflow.
#[derive(Clone, Copy)]
enum DocumentationDefect {
    MissingWith,
    PublishNotRequested,
    PublishQuoted,
    DefaultAuthorizesPublication,
}

/// Apply a semantic defect without duplicating the source fence.
fn apply_documentation_defect(
    fragment: &mut Value,
    workflow: &mut Value,
    defect: DocumentationDefect,
) -> Result<()> {
    match defect {
        DocumentationDefect::MissingWith => {
            let removed = fragment
                .as_mapping_mut()
                .context("example fragment should be a mapping")?
                .remove(Value::String("with".to_owned()));
            ensure!(
                removed.is_some(),
                "example fragment should have a `with` key"
            );
            Ok(())
        }
        DocumentationDefect::PublishNotRequested => {
            *documented_publish_mut(fragment)? = Value::Bool(false);
            Ok(())
        }
        DocumentationDefect::PublishQuoted => {
            *documented_publish_mut(fragment)? = Value::String("true".to_owned());
            Ok(())
        }
        DocumentationDefect::DefaultAuthorizesPublication => {
            let workflow_input = mapping_value_mut(workflow, "on")
                .and_then(|on| mapping_value_mut(on, "workflow_call"))
                .and_then(|call| mapping_value_mut(call, "inputs"))
                .and_then(|inputs| mapping_value_mut(inputs, INPUT_NAME))
                .context("release workflow should declare the reusable input")?;
            let default = mapping_value_mut(workflow_input, "default")
                .context("release workflow input should have a default")?;
            *default = Value::Bool(true);
            Ok(())
        }
    }
}

/// Find the documented `with.publish` value in a mutable fragment.
fn documented_publish_mut(fragment: &mut Value) -> Result<&mut Value> {
    mapping_value_mut(fragment, "with")
        .and_then(Value::as_mapping_mut)
        .and_then(|with| with.get_mut(Value::String(INPUT_NAME.to_owned())))
        .context("example fragment should contain `with.publish`")
}

/// Keep the documented publication example aligned with the reusable workflow.
#[test]
fn documented_publish_fragment_matches_the_workflow_input() -> Result<()> {
    let (example, fragment, workflow) = release_example_and_workflow()?;
    validate_publish_example(&example, &fragment, &workflow)
}

/// Reject fragments or workflows that no longer support the documented claim.
#[rstest]
#[case::missing_with(DocumentationDefect::MissingWith, "`with` mapping")]
#[case::publish_not_requested(DocumentationDefect::PublishNotRequested, "should be `true`")]
#[case::publish_quoted(DocumentationDefect::PublishQuoted, "YAML boolean")]
#[case::default_authorizes(
    DocumentationDefect::DefaultAuthorizesPublication,
    "should default to `false`"
)]
fn invalid_publish_documentation_fails_the_contract(
    #[case] defect: DocumentationDefect,
    #[case] expected_message: &str,
) -> Result<()> {
    let (example, mut fragment, mut workflow) = release_example_and_workflow()?;
    apply_documentation_defect(&mut fragment, &mut workflow, defect)?;
    let error = validate_publish_example(&example, &fragment, &workflow)
        .expect_err("invalid release publication documentation should fail its contract");
    ensure!(
        error.to_string().contains(expected_message),
        "expected {expected_message:?} in {error}"
    );
    Ok(())
}
