//! Validate CI workflow wiring for formal-verification smoke checks.

mod common;

use anyhow::{Context, Result, ensure};
use common::workflow_contents;
use serde_yaml::{Mapping, Value};

fn mapping_get<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a Value> {
    mapping.get(Value::String(key.to_owned()))
}

fn value_mapping<'a>(value: &'a Value, context: &str) -> Result<&'a Mapping> {
    value
        .as_mapping()
        .with_context(|| format!("{context} should be a mapping"))
}

fn job<'a>(workflow: &'a Value, name: &str) -> Result<&'a Mapping> {
    let root = value_mapping(workflow, "workflow")?;
    let jobs = mapping_get(root, "jobs")
        .context("workflow should define jobs")
        .and_then(|value| value_mapping(value, "jobs"))?;
    let job_value =
        mapping_get(jobs, name).with_context(|| format!("workflow should define {name}"))?;
    value_mapping(job_value, name)
}

fn steps(job: &Mapping) -> Result<&Vec<Value>> {
    mapping_get(job, "steps")
        .context("job should define steps")
        .and_then(|value| {
            value
                .as_sequence()
                .context("job steps should be a sequence")
        })
}

fn step_uses(step: &Value, expected_uses: &str) -> bool {
    step.as_mapping()
        .and_then(|mapping| mapping_get(mapping, "uses"))
        .and_then(Value::as_str)
        .is_some_and(|uses| uses == expected_uses)
}

fn step_runs(step: &Value, expected_run: &str) -> bool {
    step.as_mapping()
        .and_then(|mapping| mapping_get(mapping, "run"))
        .and_then(Value::as_str)
        .is_some_and(|run| run == expected_run)
}

fn step_name<'a>(step: &'a Value, expected_name: &str) -> Option<&'a Mapping> {
    let mapping = step.as_mapping()?;
    let name = mapping_get(mapping, "name")?.as_str()?;
    (name == expected_name).then_some(mapping)
}

#[test]
fn behavioural_ci_workflow_wires_kani_smoke_job() -> Result<()> {
    let contents = workflow_contents("ci.yml").expect("CI workflow should be readable");
    let workflow: Value = serde_yaml::from_str(&contents).context("parse CI workflow YAML")?;
    let kani_job = job(&workflow, "kani-smoke")?;

    ensure!(
        mapping_get(kani_job, "if").and_then(Value::as_str)
            == Some("github.event_name == 'pull_request'"),
        "Kani smoke job should run for pull requests"
    );

    let steps = steps(kani_job)?;
    ensure!(
        steps.iter().any(|step| step_uses(
            step,
            "astral-sh/setup-uv@4cda7d73322c50eac316ad623a716f09a2db2ac7"
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
    let cache_key = mapping_get(cache_step, "with")
        .and_then(Value::as_mapping)
        .and_then(|with| mapping_get(with, "key"))
        .and_then(Value::as_str);
    ensure!(
        cache_key
            == Some("${{ runner.os }}-kani-${{ hashFiles('tools/kani/VERSION', 'Makefile') }}"),
        "Kani smoke job should cache tools using the Kani version and Makefile"
    );

    ensure!(
        steps
            .iter()
            .any(|step| step_runs(step, "make install-kani")),
        "Kani smoke job should install Kani through the Make target"
    );
    ensure!(
        steps.iter().any(|step| step_runs(step, "make kani-check")),
        "Kani smoke job should check Kani through the Make target"
    );
    Ok(())
}
