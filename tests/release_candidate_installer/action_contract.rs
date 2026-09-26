//! Structural assertions over the release-candidate composite action.

use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use serde_yaml::{Mapping, Value as YamlValue};
use test_support::fs as test_fs;

/// Return the YAML value stored under `key` in one mapping.
pub(crate) fn mapping_value<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a YamlValue> {
    mapping.get(YamlValue::String(key.to_owned()))
}

/// Parse the release-candidate composite action.
pub(crate) fn release_candidate_action() -> Result<YamlValue> {
    let contents = test_fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".github/actions/install-release-candidate/action.yml"),
    )
    .context("read release-candidate composite action")?;

    serde_yaml::from_str(&contents).context("parse release-candidate composite action")
}

/// Require the composite action to expose both required candidate inputs.
pub(crate) fn require_candidate_inputs(root: &Mapping) -> Result<()> {
    let inputs = mapping_value(root, "inputs")
        .and_then(YamlValue::as_mapping)
        .context("release-candidate composite action should declare inputs")?;

    ensure!(
        inputs.len() == 2
            && mapping_value(inputs, "revision")
                .and_then(YamlValue::as_mapping)
                .and_then(|input| mapping_value(input, "required"))
                .and_then(YamlValue::as_bool)
                == Some(true)
            && mapping_value(inputs, "expected-version")
                .and_then(YamlValue::as_mapping)
                .and_then(|input| mapping_value(input, "required"))
                .and_then(YamlValue::as_bool)
                == Some(true),
        "the composite action should require its revision and expected-version inputs"
    );

    Ok(())
}

/// Require the composite action to map its inputs into the installer step.
pub(crate) fn require_candidate_installer_step(root: &Mapping) -> Result<()> {
    let runs = mapping_value(root, "runs")
        .and_then(YamlValue::as_mapping)
        .context("release-candidate composite action should declare runs")?;
    let steps = mapping_value(runs, "steps")
        .and_then(YamlValue::as_sequence)
        .context("release-candidate composite action should declare steps")?;
    let install_step = steps
        .first()
        .and_then(YamlValue::as_mapping)
        .context("release-candidate composite action should declare an install step")?;
    let environment = mapping_value(install_step, "env")
        .and_then(YamlValue::as_mapping)
        .context("release-candidate install step should declare an environment")?;

    ensure!(
        mapping_value(install_step, "id").and_then(YamlValue::as_str) == Some("install")
            && mapping_value(install_step, "run").and_then(YamlValue::as_str)
                == Some("bash \"${GITHUB_ACTION_PATH}/install.sh\""),
        "the composite action should invoke the installer covered by this harness"
    );
    ensure!(
        environment.len() == 2
            && mapping_value(environment, "NETSUKE_CANDIDATE_REVISION").and_then(YamlValue::as_str)
                == Some("${{ inputs.revision }}")
            && mapping_value(environment, "NETSUKE_CANDIDATE_VERSION").and_then(YamlValue::as_str)
                == Some("${{ inputs.expected-version }}"),
        "the composite action should map both candidate inputs into the installer environment"
    );

    Ok(())
}

/// Require the composite action to expose every verified installer output.
pub(crate) fn require_candidate_outputs(root: &Mapping) -> Result<()> {
    let outputs = mapping_value(root, "outputs")
        .and_then(YamlValue::as_mapping)
        .context("release-candidate composite action should declare outputs")?;

    ensure!(
        outputs.len() == 3
            && mapping_value(outputs, "binary")
                .and_then(YamlValue::as_mapping)
                .and_then(|output| mapping_value(output, "value"))
                .and_then(YamlValue::as_str)
                == Some("${{ steps.install.outputs.binary }}")
            && mapping_value(outputs, "revision")
                .and_then(YamlValue::as_mapping)
                .and_then(|output| mapping_value(output, "value"))
                .and_then(YamlValue::as_str)
                == Some("${{ steps.install.outputs.revision }}")
            && mapping_value(outputs, "version")
                .and_then(YamlValue::as_mapping)
                .and_then(|output| mapping_value(output, "value"))
                .and_then(YamlValue::as_str)
                == Some("${{ steps.install.outputs.version }}"),
        "the composite action should expose every verified installer output"
    );

    Ok(())
}
