//! Semantic contracts for reusable release-workflow examples in the guide.

use super::documentation_examples::{DocumentedExample, documented_example};
use anyhow::{Context, Result, ensure};
use rstest::rstest;
use serde_yaml::Value;

const EXAMPLE_ID: &str = "guide-release-wix-extension-version";
const INPUT_NAME: &str = "wix-extension-version";
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
    let yaml_key = Value::String(key.to_owned());
    value.as_mapping()?.get(&yaml_key)
}

/// Return a mutable string-keyed value from a YAML mapping.
fn mapping_value_mut<'a>(value: &'a mut Value, key: &str) -> Option<&'a mut Value> {
    let yaml_key = Value::String(key.to_owned());
    value.as_mapping_mut()?.get_mut(&yaml_key)
}

/// Validate that a documented workflow fragment matches the reusable input.
///
/// # Errors
///
/// Returns an error when the example is not YAML, the fragment lacks the
/// expected mapping or string value, or the workflow declares an incompatible
/// input type or default.
fn validate_release_input_example(
    example: &DocumentedExample,
    fragment: &Value,
    workflow: &Value,
) -> Result<()> {
    ensure!(
        example.language == "yaml",
        "release input example should use a YAML fence, got {:?}",
        example.language
    );

    let with = mapping_value(fragment, "with")
        .and_then(Value::as_mapping)
        .context("workflow fragment should contain a `with` mapping")?;
    let input_key = Value::String(INPUT_NAME.to_owned());
    let version = with
        .get(&input_key)
        .and_then(Value::as_str)
        .context("`with.wix-extension-version` should be a string")?;

    let workflow_input = mapping_value(workflow, "on")
        .and_then(|on| mapping_value(on, "workflow_call"))
        .and_then(|call| mapping_value(call, "inputs"))
        .and_then(|inputs| mapping_value(inputs, INPUT_NAME))
        .context("release workflow should declare the reusable input")?;
    let input_type = mapping_value(workflow_input, "type")
        .and_then(Value::as_str)
        .context("release workflow input should declare a type")?;
    ensure!(
        input_type == "string",
        "release workflow input type should be `string`, got {input_type:?}"
    );
    let default = mapping_value(workflow_input, "default")
        .and_then(Value::as_str)
        .context("release workflow input should have a string default")?;
    ensure!(
        version == default,
        "documented input value {version:?} should match workflow default {default:?}"
    );
    Ok(())
}

/// Identify one invalid edit to the parsed documentation fragment.
#[derive(Clone, Copy)]
enum FragmentDefect {
    MissingWith,
    MisspelledInput,
    NonStringVersion,
    VersionDoesNotMatchDefault,
}

/// Apply a semantic defect to parsed YAML without duplicating the source fence.
fn apply_fragment_defect(fragment: &mut Value, defect: FragmentDefect) -> Result<()> {
    match defect {
        FragmentDefect::MissingWith => remove_with_mapping(fragment),
        FragmentDefect::MisspelledInput => misspell_input_key(fragment),
        FragmentDefect::NonStringVersion => set_non_string_version(fragment),
        FragmentDefect::VersionDoesNotMatchDefault => set_mismatched_version(fragment),
    }
}

/// Remove the `with` mapping from a fragment.
fn remove_with_mapping(fragment: &mut Value) -> Result<()> {
    let with_key = Value::String("with".to_owned());
    let removed = fragment
        .as_mapping_mut()
        .context("example fragment should be a mapping")?
        .remove(&with_key);
    ensure!(
        removed.is_some(),
        "example fragment should have a `with` key"
    );
    Ok(())
}

/// Replace the reusable input key with a misspelling.
fn misspell_input_key(fragment: &mut Value) -> Result<()> {
    let with = mapping_value_mut(fragment, "with")
        .and_then(Value::as_mapping_mut)
        .context("example fragment should have a `with` mapping")?;
    let input_key = Value::String(INPUT_NAME.to_owned());
    let value = with
        .remove(&input_key)
        .context("example fragment should contain the expected input")?;
    ensure!(
        with.insert(Value::String("wix-extension-verison".to_owned()), value)
            .is_none(),
        "mutated input key should not already exist"
    );
    Ok(())
}

/// Replace the documented version with a YAML boolean.
fn set_non_string_version(fragment: &mut Value) -> Result<()> {
    *documented_version_mut(fragment)? = Value::Bool(true);
    Ok(())
}

/// Replace the documented version with a value different from the current default.
fn set_mismatched_version(fragment: &mut Value) -> Result<()> {
    *documented_version_mut(fragment)? = Value::String("6".to_owned());
    Ok(())
}

/// Find the documented input value in a mutable fragment.
fn documented_version_mut(fragment: &mut Value) -> Result<&mut Value> {
    mapping_value_mut(fragment, "with")
        .and_then(Value::as_mapping_mut)
        .and_then(|with| with.get_mut(Value::String(INPUT_NAME.to_owned())))
        .context("example fragment should contain `with.wix-extension-version`")
}

/// Keep the release input example aligned with the reusable workflow contract.
#[test]
fn documented_release_workflow_fragment_matches_the_input_default() -> Result<()> {
    let (example, fragment, workflow) = release_example_and_workflow()?;
    ensure!(
        example.language == "yaml",
        "release input example should use a YAML fence"
    );
    validate_release_input_example(&example, &fragment, &workflow)
}

/// Reject fragments that no longer describe the release workflow input.
#[rstest]
#[case::missing_with(FragmentDefect::MissingWith, "`with` mapping")]
#[case::misspelled_input(FragmentDefect::MisspelledInput, INPUT_NAME)]
#[case::non_string_version(FragmentDefect::NonStringVersion, "should be a string")]
#[case::default_mismatch(FragmentDefect::VersionDoesNotMatchDefault, "match workflow default")]
fn invalid_release_workflow_fragments_fail_the_contract(
    #[case] defect: FragmentDefect,
    #[case] expected_message: &str,
) -> Result<()> {
    let (example, mut fragment, workflow) = release_example_and_workflow()?;
    apply_fragment_defect(&mut fragment, defect)?;
    let error = validate_release_input_example(&example, &fragment, &workflow)
        .expect_err("invalid workflow fragment should fail its documentation contract");
    ensure!(
        error.to_string().contains(expected_message),
        "expected {expected_message:?} in {error}"
    );
    Ok(())
}
