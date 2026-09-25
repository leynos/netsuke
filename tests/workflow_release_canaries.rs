//! Validate how the release workflow runs the downstream migration canaries.
//!
//! The v0.1.0 release is admitted by three real downstream repositories, each
//! pinned to one commit and driven through its own Netsukefile by the exact
//! candidate. These contracts hold the release to that: publication needs every
//! canary against its own commit, the pins stay full commits that match the
//! documented table, MXD's lanes stay distinct, and a pull request is never
//! blocked by a downstream failure.

mod common;

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use common::workflow_contents;
use serde_yaml::{Mapping, Value as YamlValue};
use test_support::fs as test_fs;

/// The two canary jobs: the Linux lanes, and the ortho-config Windows gate.
const CANARY_JOBS: [&str; 2] = ["downstream-canaries", "downstream-canaries-windows"];

/// Return the YAML value stored under `key` in one mapping.
fn mapping_value<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a YamlValue> {
    mapping.get(YamlValue::String(key.to_owned()))
}

/// Parse one workflow file from `.github/workflows`.
fn workflow(name: &str) -> Result<YamlValue> {
    serde_yaml::from_str(&workflow_contents(name)?).with_context(|| format!("parse {name}"))
}

/// Return a named job from a parsed workflow.
fn job<'a>(workflow: &'a YamlValue, name: &str) -> Result<&'a Mapping> {
    workflow
        .as_mapping()
        .and_then(|root| mapping_value(root, "jobs"))
        .and_then(YamlValue::as_mapping)
        .and_then(|jobs| mapping_value(jobs, name))
        .and_then(YamlValue::as_mapping)
        .with_context(|| format!("workflow should define the {name} job"))
}

/// Return a string field from a mapping, or an empty string.
fn text<'a>(mapping: &'a Mapping, key: &str) -> &'a str {
    mapping_value(mapping, key)
        .and_then(YamlValue::as_str)
        .unwrap_or_default()
}

/// Return a job's `needs` list.
fn needs(job: &Mapping) -> Vec<&str> {
    mapping_value(job, "needs")
        .and_then(YamlValue::as_sequence)
        .map(|entries| entries.iter().filter_map(YamlValue::as_str).collect())
        .unwrap_or_default()
}

/// Return every matrix row of a canary job.
fn canary_rows(job: &Mapping) -> Result<Vec<&Mapping>> {
    mapping_value(job, "strategy")
        .and_then(YamlValue::as_mapping)
        .and_then(|strategy| mapping_value(strategy, "matrix"))
        .and_then(YamlValue::as_mapping)
        .and_then(|matrix| mapping_value(matrix, "include"))
        .and_then(YamlValue::as_sequence)
        .context("canary job should declare matrix rows")?
        .iter()
        .map(|row| row.as_mapping().context("matrix row should be a mapping"))
        .collect()
}

/// Return every `(repository, ref)` pin across both canary jobs.
fn workflow_pins(release: &YamlValue) -> Result<BTreeSet<(String, String)>> {
    let mut pins = BTreeSet::new();
    for name in CANARY_JOBS {
        for row in canary_rows(job(release, name)?)? {
            pins.insert((
                text(row, "repository").to_owned(),
                text(row, "ref").to_owned(),
            ));
        }
    }
    Ok(pins)
}

/// Return every `(repository, ref)` pin in the documented canary table.
fn documented_pins() -> Result<BTreeSet<(String, String)>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join("release-admission-canaries.md");
    let contents = test_fs::read_to_string(&path).context("read the canary documentation")?;
    Ok(contents
        .lines()
        .filter(|line| line.starts_with("| `leynos/"))
        .filter_map(|line| {
            let cells: Vec<&str> = line
                .split('|')
                .map(|cell| cell.trim().trim_matches('`'))
                .collect();
            Some((cells.get(1)?.to_string(), cells.get(2)?.to_string()))
        })
        .collect())
}

/// Return whether `value` is a full lowercase 40-hex commit.
fn is_full_commit(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

#[test]
fn publication_requires_every_canary_against_its_own_commit() -> Result<()> {
    let release_workflow = workflow("release.yml")?;
    let release = job(&release_workflow, "release")?;
    let condition = text(release, "if");

    for required in ["release-candidate", CANARY_JOBS[0], CANARY_JOBS[1]] {
        ensure!(
            needs(release).contains(&required),
            "release should need {required}"
        );
    }
    for clause in [
        "needs.metadata.outputs.should_publish == 'true'",
        "needs.downstream-canaries.result == 'success'",
        "needs.downstream-canaries-windows.result == 'success'",
        "needs.release-candidate.outputs.commit == github.sha",
    ] {
        ensure!(
            condition.contains(clause),
            "release should require `{clause}`"
        );
    }
    ensure!(
        !condition.contains("||"),
        "no clause may bypass the others: {condition}"
    );
    Ok(())
}

#[test]
fn every_canary_is_pinned_to_a_documented_full_commit() -> Result<()> {
    let pins = workflow_pins(&workflow("release.yml")?)?;
    let repositories: BTreeSet<&str> = pins
        .iter()
        .map(|(repository, _)| repository.as_str())
        .collect();

    ensure!(
        repositories
            == BTreeSet::from([
                "leynos/mxd",
                "leynos/ortho-config",
                "leynos/repovec-appliance"
            ]),
        "the canaries should cover exactly the three downstream repositories: {repositories:?}"
    );
    ensure!(
        pins.len() == 3,
        "each repository should have one pin across its rows: {pins:?}"
    );
    for (repository, reference) in &pins {
        ensure!(
            is_full_commit(reference),
            "{repository} should pin a full commit, got {reference}"
        );
    }
    ensure!(
        pins == documented_pins()?,
        "the workflow pins should match the documented table"
    );
    Ok(())
}

#[test]
fn mxd_lanes_are_distinct_and_isolated() -> Result<()> {
    let release_workflow = workflow("release.yml")?;
    let rows = canary_rows(job(&release_workflow, CANARY_JOBS[0])?)?;
    let lanes: Vec<&Mapping> = rows
        .into_iter()
        .filter(|row| text(row, "repository") == "leynos/mxd")
        .collect();
    let selectors: BTreeSet<&str> = lanes.iter().map(|row| text(row, "selectors")).collect();

    ensure!(
        selectors
            == BTreeSet::from([
                "MXD_BACKEND=postgres",
                "MXD_BACKEND=sqlite",
                "MXD_BACKEND=wireframe-only"
            ]),
        "MXD should run one row per lane: {selectors:?}"
    );
    for row in lanes {
        ensure!(
            !text(row, "forbid").trim().is_empty(),
            "every MXD lane should forbid the others"
        );
        ensure!(
            mapping_value(row, "postgres").and_then(YamlValue::as_bool)
                == Some(text(row, "selectors") == "MXD_BACKEND=postgres"),
            "only the PostgreSQL lane should start the database service"
        );
    }
    Ok(())
}

#[test]
fn canaries_run_on_github_hosted_runners_and_never_gate_pull_requests() -> Result<()> {
    let release_workflow = workflow("release.yml")?;
    let candidate = job(&release_workflow, "release-candidate")?;

    ensure!(
        text(candidate, "if")
            == "github.event_name != 'pull_request' || github.event.action == 'ready_for_review'",
        "a pull request should run the canaries only when marked ready for review"
    );
    for (name, runner) in [
        (CANARY_JOBS[0], "ubuntu-latest"),
        (CANARY_JOBS[1], "windows-latest"),
    ] {
        let canary = job(&release_workflow, name)?;
        ensure!(
            text(canary, "runs-on") == runner,
            "{name} should run on {runner}"
        );
        ensure!(
            needs(canary).contains(&"release-candidate"),
            "{name} should build the resolved candidate"
        );
        ensure!(
            text(canary, "continue-on-error") == "${{ github.event_name == 'pull_request' }}",
            "{name} should never gate a pull request"
        );
    }
    Ok(())
}

#[test]
fn release_build_jobs_request_only_the_checkout_read_scope() -> Result<()> {
    let release_workflow = workflow("release.yml")?;
    for name in ["build-linux", "build-windows", "build-macos"] {
        let permissions = mapping_value(job(&release_workflow, name)?, "permissions")
            .and_then(YamlValue::as_mapping)
            .with_context(|| format!("{name} should declare permissions"))?;
        ensure!(
            permissions.len() == 1 && text(permissions, "contents") == "read",
            "{name} should request only contents read permission"
        );
    }
    Ok(())
}

#[test]
fn dry_run_offers_a_manual_rehearsal_of_the_whole_release() -> Result<()> {
    let dry_run = workflow("release-dry-run.yml")?;
    let triggers = dry_run
        .as_mapping()
        .and_then(|root| mapping_value(root, "on"))
        .and_then(YamlValue::as_mapping)
        .context("the dry run should declare triggers")?;
    let dispatch_input = mapping_value(triggers, "workflow_dispatch")
        .and_then(YamlValue::as_mapping)
        .and_then(|dispatch| mapping_value(dispatch, "inputs"))
        .and_then(YamlValue::as_mapping)
        .and_then(|inputs| mapping_value(inputs, "candidate-ref"));
    let release = job(&dry_run, "release")?;
    let with = mapping_value(release, "with")
        .and_then(YamlValue::as_mapping)
        .context("the dry run should configure the release workflow")?;

    ensure!(
        dispatch_input.is_some(),
        "a manual rehearsal should accept a candidate ref"
    );
    ensure!(
        mapping_value(with, "dry-run").and_then(YamlValue::as_bool) == Some(true),
        "a rehearsal must never publish"
    );
    ensure!(
        text(with, "candidate-ref") == "${{ inputs.candidate-ref || '' }}",
        "a rehearsal should pass its candidate ref to the canaries"
    );
    Ok(())
}
