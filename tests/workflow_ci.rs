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

fn workflow_env(workflow: &Value, key: YamlKey) -> Result<&str> {
    let root = value_mapping(workflow, "workflow")?;
    let env = mapping_get(root, YamlKey("env"))
        .context("workflow should define a workflow-level env")
        .and_then(|value| value_mapping(value, "workflow-level env"))?;
    mapping_get(env, key)
        .and_then(Value::as_str)
        .with_context(|| format!("workflow-level env should define {}", key.0))
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

fn named_step<'a>(steps: &'a [Value], name: &str) -> Result<&'a Mapping> {
    steps
        .iter()
        .find_map(|step| step_name(step, name))
        .with_context(|| format!("job should include the {name} step"))
}

fn step_index(steps: &[Value], name: &str) -> Result<usize> {
    steps
        .iter()
        .position(|step| step_name(step, name).is_some())
        .with_context(|| format!("job should include the {name} step"))
}

fn step_input(step: &Mapping, key: YamlKey) -> Option<&str> {
    mapping_get(step, YamlKey("with"))
        .and_then(Value::as_mapping)
        .and_then(|with| mapping_get(with, key))
        .and_then(Value::as_str)
}

fn job_env(job: &Mapping, key: YamlKey) -> Option<&str> {
    mapping_get(job, YamlKey("env"))
        .and_then(Value::as_mapping)
        .and_then(|env| mapping_get(env, key))
        .and_then(Value::as_str)
}

/// Returns true when `reference` is `<action>@<40-character lowercase-hex SHA>`.
///
/// Dependabot owns the SHA value, so contract tests assert the shape of the
/// pin rather than a literal commit; see "Workflow pins and Dependabot" in
/// `docs/developers-guide.md`.
fn is_pinned_action_ref(reference: &str, action: &str) -> bool {
    let Some(pin) = reference
        .strip_prefix(action)
        .and_then(|rest| rest.strip_prefix('@'))
    else {
        return false;
    };
    pin.len() == 40
        && pin
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

/// Returns true when `version` is an exact `major.minor.patch` pin.
fn is_exact_version(version: &str) -> bool {
    let mut parts = version.split('.');
    let numeric = |component: Option<&str>| {
        component.is_some_and(|value| {
            !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
        })
    };
    numeric(parts.next())
        && numeric(parts.next())
        && numeric(parts.next())
        && parts.next().is_none()
}

#[test]
fn unit_recognizes_pinned_action_refs() {
    assert!(is_pinned_action_ref(
        "taiki-e/install-action@0123456789abcdef0123456789abcdef01234567",
        "taiki-e/install-action"
    ));
    assert!(!is_pinned_action_ref(
        "taiki-e/install-action@v2",
        "taiki-e/install-action"
    ));
    assert!(!is_pinned_action_ref(
        "taiki-e/install-action@0123456789ABCDEF0123456789ABCDEF01234567",
        "taiki-e/install-action"
    ));
    assert!(!is_pinned_action_ref(
        "someone-else/install-action@0123456789abcdef0123456789abcdef01234567",
        "taiki-e/install-action"
    ));
}

#[test]
fn unit_recognizes_exact_versions() {
    assert!(is_exact_version("0.9.133"));
    assert!(!is_exact_version("0.9"));
    assert!(!is_exact_version("0.9.133.1"));
    assert!(!is_exact_version("latest"));
    assert!(!is_exact_version("^0.9.133"));
}

#[test]
fn behavioural_ci_workflow_installs_pinned_cargo_nextest() -> Result<()> {
    let contents = workflow_contents("ci.yml").expect("CI workflow should be readable");
    let workflow: Value = serde_yaml::from_str(&contents).context("parse CI workflow YAML")?;

    let version = workflow_env(&workflow, YamlKey("NEXTEST_VERSION"))
        .context("workflow-level env should pin NEXTEST_VERSION")?;
    ensure!(
        is_exact_version(version),
        "NEXTEST_VERSION should pin an exact version, found {version:?}"
    );

    for job_name in ["build-test", "build-test-windows"] {
        let build_test = job(&workflow, job_name)?;
        ensure!(
            job_env(build_test, YamlKey("NEXTEST_VERSION")).is_none(),
            "{job_name} should not duplicate NEXTEST_VERSION at job scope"
        );

        let steps = steps(build_test)?;
        let install = named_step(steps, "Install cargo-nextest")?;
        let uses = mapping_get(install, YamlKey("uses"))
            .and_then(Value::as_str)
            .context("Install cargo-nextest step should reference an action")?;
        ensure!(
            is_pinned_action_ref(uses, "taiki-e/install-action"),
            "cargo-nextest installer should be pinned to a full commit SHA, found {uses:?}"
        );
        ensure!(
            step_input(install, YamlKey("tool")) == Some("nextest@${{ env.NEXTEST_VERSION }}"),
            "cargo-nextest installer should resolve its pin from NEXTEST_VERSION"
        );
        ensure!(
            !install.contains_key(Value::String("if".to_owned())),
            "cargo-nextest should install on every matrix leg because every leg runs make test"
        );
    }
    Ok(())
}

#[test]
fn behavioural_ci_workflow_runs_tests_through_the_make_target() -> Result<()> {
    let contents = workflow_contents("ci.yml").expect("CI workflow should be readable");
    let workflow: Value = serde_yaml::from_str(&contents).context("parse CI workflow YAML")?;
    let build_test = job(&workflow, "build-test")?;
    let steps = steps(build_test)?;

    let test_step = named_step(steps, "Test")?;
    ensure!(
        mapping_get(test_step, YamlKey("run")).and_then(Value::as_str) == Some("make test"),
        "the Test step should run the canonical make target"
    );
    ensure!(
        step_index(steps, "Install cargo-nextest")? < step_index(steps, "Test")?,
        "cargo-nextest should be installed before make test runs"
    );
    let setup_uv = named_step(steps, "Setup uv")?;
    ensure!(
        step_input(setup_uv, YamlKey("enable-cache")) == Some("false"),
        "the manifest-free spelling job must not enable uv's automatic cache"
    );
    Ok(())
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
    let uv_cache_enabled = mapping_get(install_uv_step, YamlKey("with"))
        .and_then(Value::as_mapping)
        .and_then(|with| mapping_get(with, YamlKey("enable-cache")));
    ensure!(
        uv_cache_enabled == Some(&Value::Bool(false)),
        "Install uv step should disable automatic caching because Kani uses an explicit cache"
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

    let install_kani_index = steps
        .iter()
        .position(|step| step_has(step, StepField::Runs, "make install-kani"))
        .context("Kani smoke job should install Kani through the Make target")?;
    let kani_check_index = steps
        .iter()
        .position(|step| step_has(step, StepField::Runs, "make kani-check"))
        .context("Kani smoke job should check Kani through the Make target")?;
    let kani_ir_index = steps
        .iter()
        .position(|step| step_has(step, StepField::Runs, "make kani-ir"))
        .context("Kani smoke job should run the bounded Kani harnesses through the Make target")?;
    ensure!(
        install_kani_index < kani_check_index && kani_check_index < kani_ir_index,
        "Kani smoke job should install Kani, check its version, then run the bounded harnesses"
    );
    ensure!(
        mapping_get(kani_job, YamlKey("timeout-minutes")).and_then(Value::as_u64) == Some(20),
        "Kani smoke job should enforce the 20-minute cold-run ceiling"
    );
    Ok(())
}
