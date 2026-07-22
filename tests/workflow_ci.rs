//! Validate CI workflow wiring for formal-verification smoke checks.

mod common;

use anyhow::{Context, Result, ensure};
use common::workflow_contents;
use serde_yaml::{Mapping, Value};

#[derive(Clone, Copy)]
struct YamlKey(pub &'static str);

#[derive(Clone, Copy)]
enum StepField {
    Uses,
    Runs,
}

impl StepField {
    const fn yaml_key(self) -> &'static str {
        match self {
            Self::Uses => "uses",
            Self::Runs => "run",
        }
    }
}

fn mapping_get(mapping: &Mapping, key: YamlKey) -> Option<&Value> {
    mapping.get(Value::String(key.0.to_owned()))
}

fn value_mapping<'a>(value: &'a Value, context: &str) -> Result<&'a Mapping> {
    value
        .as_mapping()
        .with_context(|| format!("{context} should be a mapping"))
}

fn job<'a>(workflow: &'a Value, name: &'static str) -> Result<&'a Mapping> {
    let root = value_mapping(workflow, "workflow")?;
    let jobs = mapping_get(root, YamlKey("jobs"))
        .context("workflow should define jobs")
        .and_then(|value| value_mapping(value, "jobs"))?;
    // `YamlKey` is intentionally static-only; do not pass dynamic job names.
    let job_value = mapping_get(jobs, YamlKey(name))
        .with_context(|| format!("workflow should define {name}"))?;
    value_mapping(job_value, name)
}

fn steps(job: &Mapping) -> Result<&Vec<Value>> {
    mapping_get(job, YamlKey("steps"))
        .context("job should define steps")
        .and_then(|value| {
            value
                .as_sequence()
                .context("job steps should be a sequence")
        })
}

fn step_has(step: &Value, field: StepField, expected: &str) -> bool {
    step.as_mapping()
        .and_then(|mapping| mapping_get(mapping, YamlKey(field.yaml_key())))
        .and_then(Value::as_str)
        .is_some_and(|value| value == expected)
}

fn step_name<'a>(step: &'a Value, expected_name: &str) -> Option<&'a Mapping> {
    let mapping = step.as_mapping()?;
    let name = mapping_get(mapping, YamlKey("name"))?.as_str()?;
    (name == expected_name).then_some(mapping)
}

#[test]
fn behavioural_ci_workflow_wires_kani_smoke_job() -> Result<()> {
    let contents = workflow_contents("ci.yml").expect("CI workflow should be readable");
    let workflow: Value = serde_yaml::from_str(&contents).context("parse CI workflow YAML")?;
    let kani_job = job(&workflow, "kani-smoke")?;

    ensure!(
        mapping_get(kani_job, YamlKey("if")).and_then(Value::as_str)
            == Some("github.event_name == 'pull_request'"),
        "Kani smoke job should run for pull requests"
    );

    let steps = steps(kani_job)?;
    ensure!(
        steps.iter().any(|step| step_has(
            step,
            StepField::Uses,
            "astral-sh/setup-uv@11f9893b081a58869d3b5fccaea48c9e9e46f990"
        )),
        "Kani smoke job should install uv with the pinned setup-uv action"
    );

    let install_uv_step = steps
        .iter()
        .find_map(|step| step_name(step, "Install uv"))
        .context("Kani smoke job should include the Install uv step")?;
    ensure!(
        !install_uv_step.contains_key(Value::String("with".to_owned())),
        "Install uv step should not include a with configuration"
    );

    let cache_step = steps
        .iter()
        .find_map(|step| step_name(step, "Cache Kani tools"))
        .context("Kani smoke job should include the Cache Kani tools step")?;
    let cache_key = mapping_get(cache_step, YamlKey("with"))
        .and_then(Value::as_mapping)
        .and_then(|with| mapping_get(with, YamlKey("key")))
        .and_then(Value::as_str);
    ensure!(
        cache_key
            == Some("${{ runner.os }}-kani-${{ hashFiles('tools/kani/VERSION', 'Makefile') }}"),
        "Kani smoke job should cache tools using the Kani version and Makefile"
    );

    ensure!(
        steps
            .iter()
            .any(|step| step_has(step, StepField::Runs, "make install-kani")),
        "Kani smoke job should install Kani through the Make target"
    );
    ensure!(
        steps
            .iter()
            .any(|step| step_has(step, StepField::Runs, "make kani-check")),
        "Kani smoke job should check Kani through the Make target"
    );
    Ok(())
}
