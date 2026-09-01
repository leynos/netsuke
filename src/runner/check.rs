//! Dispatch and rendering for the `netsuke check` subcommand.
//!
//! The manifest is loaded, expanded, rendered, and lowered through the same
//! stages as a build, with impure template helpers disabled, and the resulting
//! artefacts are handed to the linter. No recipe runs and no build output is
//! created.
//!
//! Findings are the command's result rather than its failure mode: the
//! `--fail-on` threshold decides whether they are reported as a successful
//! result document or as a failure diagnostic. See
//! `docs/adr-018-manifest-linting-under-netsuke-check.md`.

use anyhow::{Context, Result};

use crate::cli::{CheckArgs, Cli};
use crate::ir::BuildGraph;
use crate::lint::{self, Bounds, FailOn, Policy, PolicyError};
use crate::localization::{self, keys};
use crate::status::{LocalizationKey, PipelineStage, StatusReporter, report_pipeline_stage};

use super::check_diagnostics::CheckReport;
use super::check_telemetry::{self, CheckFailure};
use super::error::RunnerError;
use super::generation;
use super::path_helpers::{ensure_manifest_exists, resolve_manifest_path};
use super::process;

#[path = "check_explain.rs"]
mod explain;
#[path = "check_json.rs"]
mod json;
#[path = "check_text.rs"]
mod text;

/// Lint the selected manifest, or print the rule reference.
///
/// # Errors
///
/// Returns an error when a policy selector is invalid, when the manifest
/// cannot be resolved, loaded, rendered, or lowered, or when findings reach
/// the failure threshold.
pub(super) fn handle_check(
    cli: &Cli,
    args: &CheckArgs,
    reporter: &dyn StatusReporter,
) -> Result<()> {
    check_telemetry::instrument_check(|| handle_check_inner(cli, args, reporter))
}

/// Execute the check flow while classifying failures for boundary telemetry.
fn handle_check_inner(
    cli: &Cli,
    args: &CheckArgs,
    reporter: &dyn StatusReporter,
) -> Result<(), CheckFailure> {
    if let Some(rule) = args.explain.as_deref() {
        return explain::render(cli, rule).map_err(CheckFailure::Output);
    }
    let policy = resolve_policy(&args.rule).map_err(CheckFailure::Policy)?;
    let threshold = parse_threshold(&args.fail_on).map_err(CheckFailure::Policy)?;
    let bounds = Bounds {
        limit: args.limit,
        threshold,
    };
    let report = analyse(cli, reporter, &policy, bounds).map_err(CheckFailure::Analysis)?;
    emit(cli, &report).map_err(CheckFailure::Output)?;
    finish(&report).map_err(CheckFailure::Threshold)?;
    reporter.report_complete(status_key());
    Ok(())
}

/// Load the manifest, lower it, and lint the result.
fn analyse(
    cli: &Cli,
    reporter: &dyn StatusReporter,
    policy: &Policy,
    bounds: Bounds,
) -> Result<CheckReport> {
    let path = resolve_manifest_path(cli)?;
    ensure_manifest_exists(cli, &path)?;
    let mut on_stage = super::stage_reporting_callback(reporter);
    let loaded = generation::load_manifest_with_source(&path, Some(&mut on_stage))?;
    let graph = BuildGraph::from_manifest(&loaded.manifest)
        .context(localization::message(keys::RUNNER_CONTEXT_BUILD_GRAPH))?;
    report_pipeline_stage(reporter, PipelineStage::IrGenerationValidation, None);
    let outcome = lint::analyse(
        lint::Request {
            source: loaded.source.clone(),
            manifest: &loaded.manifest,
            graph: &graph,
        },
        policy,
    )
    .map_err(|failure| RunnerError::CheckSourceIndex {
        message: localization::message(keys::CHECK_SOURCE_INDEX)
            .with_arg("path", loaded.name.clone())
            .with_arg("line", failure.line.to_string())
            .with_arg("reason", failure.message),
    })?;
    Ok(CheckReport::new(
        &loaded.name,
        loaded.source,
        outcome,
        bounds,
    ))
}

/// Write the report in whichever mode the caller selected.
///
/// A failing report writes nothing here: the threshold diagnostic carries
/// every finding, so rendering them twice would duplicate the output in human
/// mode and would put a result document on stdout in JSON mode.
fn emit(cli: &Cli, report: &CheckReport) -> Result<()> {
    if report.report().is_failure() {
        return Ok(());
    }
    let rendered = if cli.json {
        json::render_result(report)?
    } else {
        text::render(report, cli)
    };
    process::write_text_stdout(&rendered)
}

/// Convert the report into the command's outcome.
fn finish(report: &CheckReport) -> Result<()> {
    let domain_report = report.report();
    if !domain_report.is_failure() {
        return Ok(());
    }
    let message = localization::message(keys::CHECK_THRESHOLD_EXCEEDED)
        .with_arg("severity", domain_report.threshold().as_str())
        .with_arg("reported", domain_report.findings().len().to_string())
        .with_arg("failing", domain_report.failing_count().to_string());
    Err(RunnerError::LintThresholdExceeded {
        message,
        help: localization::message(keys::CHECK_THRESHOLD_EXCEEDED_HELP),
        findings: report.diagnostics(),
    }
    .into())
}

/// The localization key naming this command in status output.
fn status_key() -> LocalizationKey {
    keys::STATUS_TOOL_CHECK.into()
}

/// Resolve the policy selectors, localizing any rejection.
fn resolve_policy(selectors: &[String]) -> Result<Policy> {
    Policy::resolve(selectors).map_err(|error| {
        RunnerError::CheckPolicy {
            message: localize_policy_error(&error),
        }
        .into()
    })
}

/// Render a policy failure through the localization catalogue.
fn localize_policy_error(error: &PolicyError) -> localization::LocalizedMessage {
    match error {
        PolicyError::Malformed { selector } => {
            localization::message(keys::CHECK_RULE_MALFORMED).with_arg("selector", selector.clone())
        }
        PolicyError::UnknownName { name } => {
            localization::message(keys::CHECK_RULE_UNKNOWN).with_arg("name", name.clone())
        }
        PolicyError::UnknownSeverity { name, severity } => {
            localization::message(keys::CHECK_RULE_SEVERITY)
                .with_arg("name", name.clone())
                .with_arg("severity", severity.clone())
                .with_arg("values", lint::SEVERITY_VALUES)
        }
    }
}

/// Parse the failure threshold, localizing any rejection.
fn parse_threshold(value: &str) -> Result<FailOn> {
    value.parse::<FailOn>().map_err(|rejected| {
        RunnerError::CheckPolicy {
            message: localization::message(keys::CHECK_FAIL_ON_INVALID)
                .with_arg("value", rejected)
                .with_arg("values", lint::FAIL_ON_VALUES),
        }
        .into()
    })
}

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;
