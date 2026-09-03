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

const INSTALL_NIXIE_ACTION: &str = "leynos/shared-actions/.github/actions/install-nixie";
const INSTALL_WHITAKER_ACTION: &str = "leynos/shared-actions/.github/actions/install-whitaker";
const WHITAKER_INSTALLER_VERSION: &str = "0.2.7";

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

/// Path of the composite action that owns the Kani job's cache entry.
///
/// The cache steps live in an action rather than inline so `ci.yml` stays
/// inside the repository's 400-line file limit; see "Cache ownership and
/// bounded CI resources" in `docs/developers-guide.md`.
const KANI_CACHE_ACTION: &str = "./.github/actions/kani-cache";

fn ensure_kani_cache_contract(steps: &[Value]) -> Result<()> {
    let restore_step = named_step(steps, "Restore Kani payloads")?;
    let save_step = named_step(steps, "Save Kani payloads")?;
    for (step, mode, label) in [
        (restore_step, "restore", "restore"),
        (save_step, "save", "save"),
    ] {
        ensure!(
            mapping_get(step, YamlKey("uses")).and_then(Value::as_str) == Some(KANI_CACHE_ACTION),
            "Kani smoke job should {label} through the repository's Kani cache action"
        );
        ensure!(
            step_input(step, YamlKey("mode")) == Some(mode),
            "Kani smoke job's {label} step should ask for the {mode} mode"
        );
    }
    let restore_index = step_index(steps, "Restore Kani payloads")?;
    let install_index = step_index(steps, "Install prebuilt Kani")?;
    let save_index = step_index(steps, "Save Kani payloads")?;
    ensure!(
        restore_index < install_index,
        "Kani smoke job should restore its cache before installing Kani, or a warm \
         entry cannot skip the download"
    );
    ensure!(
        install_index < save_index,
        "Kani smoke job should publish its cache only after the payloads exist"
    );
    Ok(())
}

/// Require each Kani archive's checksum to gate that same archive's use.
///
/// A bare `sha256sum --check` substring would also be satisfied by verifying
/// an unrelated file, so each assertion names the archive shell variable and
/// requires the verification to precede the extraction or setup that consumes
/// it.
fn ensure_kani_archives_are_verified_before_use(install_command: &str) -> Result<()> {
    let checked_uses = [
        (
            "\"${frontend_archive}\" | sha256sum --check --",
            "tar --extract --gzip --file \"${frontend_archive}\"",
            "front-end archive",
        ),
        (
            "\"${bundle}\" | sha256sum --check --",
            "cargo kani setup --use-local-bundle \"${bundle}\"",
            "verifier bundle",
        ),
    ];
    for (check, use_site, label) in checked_uses {
        let check_index = install_command
            .find(check)
            .with_context(|| format!("Kani {label} should be checksum-verified by name"))?;
        let use_index = install_command
            .find(use_site)
            .with_context(|| format!("Kani {label} should be unpacked by name"))?;
        ensure!(
            check_index < use_index,
            "Kani {label} should be verified before it is unpacked"
        );
    }
    Ok(())
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

    let windows_contents =
        workflow_contents("ci-windows.yml").expect("Windows CI workflow should be readable");
    let windows_workflow: Value =
        serde_yaml::from_str(&windows_contents).context("parse Windows CI workflow YAML")?;

    for (source, job_name) in [
        (&workflow, "build-test"),
        (&windows_workflow, "build-test-windows"),
    ] {
        let build_test = job(source, job_name)?;
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

    // The instrumented coverage run is the lane's only test execution, so the
    // doctests it cannot execute are the one thing left for a make target.
    let doctest_step = named_step(steps, "Doctests")?;
    ensure!(
        mapping_get(doctest_step, YamlKey("run")).and_then(Value::as_str) == Some("make doctest"),
        "the Doctests step should run the canonical make target"
    );
    ensure!(
        !steps.iter().any(|step| step_name(step, "Test").is_some()),
        "the uninstrumented test execution should be folded into the coverage run"
    );
    ensure!(
        step_index(steps, "Install cargo-nextest")?
            < step_index(steps, "Test and Measure Coverage")?,
        "cargo-nextest should be installed before the instrumented run"
    );
    let setup_uv = named_step(steps, "Setup uv")?;
    ensure!(
        step_input(setup_uv, YamlKey("enable-cache")) == Some("false"),
        "the manifest-free spelling job must not enable uv's automatic cache"
    );
    Ok(())
}

/// Assert that a job installs Whitaker through the pinned shared action.
fn ensure_shared_whitaker_installer(workflow: &Value, job_name: &'static str) -> Result<()> {
    let whitaker = named_step(steps(job(workflow, job_name)?)?, "Install Whitaker")?;
    let uses = mapping_get(whitaker, YamlKey("uses"))
        .and_then(Value::as_str)
        .context("Install Whitaker should reference the shared installer action")?;
    ensure!(
        is_pinned_action_ref(uses, INSTALL_WHITAKER_ACTION),
        "{job_name} Install Whitaker should use a commit-pinned shared installer, found {uses:?}"
    );
    ensure!(
        step_input(whitaker, YamlKey("installer-version")) == Some(WHITAKER_INSTALLER_VERSION),
        "{job_name} Install Whitaker should retain installer version {WHITAKER_INSTALLER_VERSION}"
    );
    ensure!(
        mapping_get(whitaker, YamlKey("run")).is_none(),
        "{job_name} Install Whitaker should not retain an ad hoc installation script"
    );
    Ok(())
}

/// Assert that Windows lints both packages through the PowerShell wrapper.
fn ensure_windows_whitaker_wrapper(windows_workflow: &Value) -> Result<()> {
    let lint = named_step(
        steps(job(windows_workflow, "build-test-windows")?)?,
        "Lint (Whitaker)",
    )?;
    let script = mapping_get(lint, YamlKey("run"))
        .and_then(Value::as_str)
        .context("Windows Whitaker lint should define a PowerShell script")?;
    ensure!(
        mapping_get(lint, YamlKey("shell")).and_then(Value::as_str) == Some("pwsh"),
        "Windows Whitaker lint should run from PowerShell"
    );
    ensure!(
        script.contains("whitaker.ps1") && script.contains("Push-Location test_support"),
        "Windows Whitaker lint should run both packages through the installed PowerShell wrapper"
    );
    Ok(())
}

#[test]
fn behavioural_ci_workflow_uses_shared_tool_installers() -> Result<()> {
    let contents = workflow_contents("ci.yml").expect("CI workflow should be readable");
    let workflow: Value = serde_yaml::from_str(&contents).context("parse CI workflow YAML")?;

    let windows_contents =
        workflow_contents("ci-windows.yml").expect("Windows CI workflow should be readable");
    let windows_workflow: Value =
        serde_yaml::from_str(&windows_contents).context("parse Windows CI workflow YAML")?;

    ensure_shared_whitaker_installer(&workflow, "build-test")?;
    ensure_shared_whitaker_installer(&windows_workflow, "build-test-windows")?;
    ensure_windows_whitaker_wrapper(&windows_workflow)?;

    let linux_steps = steps(job(&workflow, "build-test")?)?;
    let nixie = named_step(linux_steps, "Install Nixie")?;
    let uses = mapping_get(nixie, YamlKey("uses"))
        .and_then(Value::as_str)
        .context("Install Nixie should reference the shared installer action")?;
    ensure!(
        is_pinned_action_ref(uses, INSTALL_NIXIE_ACTION),
        "Install Nixie should use a commit-pinned shared installer, found {uses:?}"
    );
    ensure!(
        step_input(nixie, YamlKey("python-version")) == Some("3.14"),
        "Install Nixie should use Python 3.14, which nixie-cli requires"
    );
    ensure!(
        step_index(linux_steps, "Setup uv")? < step_index(linux_steps, "Install Nixie")?
            && step_index(linux_steps, "Install Nixie")?
                < step_index(linux_steps, "Validate Mermaid diagrams")?,
        "Nixie should install after uv and before Mermaid validation"
    );
    ensure!(
        mapping_get(nixie, YamlKey("run")).is_none(),
        "Install Nixie should not retain an ad hoc installation script"
    );
    let validation = named_step(linux_steps, "Validate Mermaid diagrams")?;
    ensure!(
        mapping_get(validation, YamlKey("run")).and_then(Value::as_str) == Some("make nixie"),
        "Validate Mermaid diagrams should run the canonical make target"
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
            == Some("github.event_name != 'workflow_dispatch'"),
        "Kani smoke job should run for pull requests and for the trunk push that \
         writes its cache"
    );

    let steps = steps(kani_job)?;
    ensure!(
        !steps
            .iter()
            .any(|step| step_has(step, StepField::Uses, "astral-sh/setup-uv@")),
        "Kani smoke job installs prebuilt archives directly and needs no uv runtime"
    );

    ensure_kani_cache_contract(steps)?;

    let install_kani_index = step_index(steps, "Install prebuilt Kani")?;
    let kani_check_index = steps
        .iter()
        .position(|step| step_name(step, "Kani version check").is_some())
        .context("Kani smoke job should check the installed Kani version")?;
    let kani_ir_index = steps
        .iter()
        .position(|step| step_has(step, StepField::Runs, "make kani-ir"))
        .context("Kani smoke job should run the bounded Kani harnesses through the Make target")?;
    ensure!(
        install_kani_index < kani_check_index && kani_check_index < kani_ir_index,
        "Kani smoke job should install Kani, check its version, then run the bounded harnesses"
    );
    let install_kani = named_step(steps, "Install prebuilt Kani")?;
    let install_command = mapping_get(install_kani, YamlKey("run"))
        .and_then(Value::as_str)
        .context("Kani release installation should be a shell command")?;
    ensure!(
        !install_command.contains("cargo install"),
        "Kani smoke job should never compile the verifier from source"
    );
    ensure!(
        install_command.contains("frontend_bin=\"${CARGO_HOME}/frontend/kani-${kani_version}\"")
            && install_command.contains("kani_dir=\"${KANI_HOME}/kani-${kani_version}\""),
        "Kani payloads should live under version-qualified directories so a version bump \
         cannot be satisfied by a stale cached binary"
    );
    ensure_kani_archives_are_verified_before_use(install_command)?;
    ensure!(
        mapping_get(kani_job, YamlKey("timeout-minutes")).and_then(Value::as_u64) == Some(20),
        "Kani smoke job should enforce the 20-minute cold-run ceiling"
    );
    Ok(())
}
