//! Verify the ordinary cause-chain fallback used when no diagnostic is selected.
//!
//! The fallback exists for the failures whose only explanation is a plain
//! `anyhow` cause: graph rejections, rejected command interpolation, and Ninja
//! synthesis errors. These tests pin the ordering, deduplication, and
//! precedence rules that keep that rendering predictable.

use super::*;
use crate::human_diagnostics::cause_chain::{CauseChain, distinct_causes};
use std::error::Error as _;

/// Build a failure whose cause chain names each layer.
fn nested_failure() -> anyhow::Error {
    anyhow::Error::msg("root cause")
        .context("inner context")
        .context("outer summary")
}

/// Render the plain cause chain beneath the outer context in chain order.
///
/// Reversing the chain, or rendering only its outermost link, breaks one of the
/// three position assertions: the outer context must come first, then the
/// intermediate context, then the root cause.
#[test]
fn ordinary_causes_follow_the_context_in_order() -> Result<()> {
    let report = failure_report(
        &nested_failure(),
        &theme_for(presentation_style(false, true)),
    )
    .context("a failure with ordinary causes should report")?;
    let outer = report
        .find("outer summary")
        .context("the report must start from the outer context")?;
    let inner = report
        .find("inner context")
        .context("the report must include the intermediate context")?;
    let root = report
        .find("root cause")
        .context("the report must include the root cause")?;
    ensure!(
        outer < inner && inner < root,
        "causes must render in chain order from outer to root: {report}"
    );
    Ok(())
}

/// A repeated cause is printed once, not once per link that repeats it.
///
/// The outer context is excluded from the rendering, and the second copy of the
/// cause is dropped because it repeats its predecessor. A renderer that kept
/// every link would print the sentence twice.
#[test]
fn repeated_causes_are_printed_once() -> Result<()> {
    let error = anyhow::Error::msg("repeated cause")
        .context("repeated cause")
        .context("outer summary");
    let report = failure_report(&error, &theme_for(presentation_style(false, true)))
        .context("a failure with a repeated cause should report")?;
    ensure!(
        report.matches("repeated cause").count() == 1,
        "a repeated cause must be printed exactly once: {report}"
    );
    Ok(())
}

/// A cause that merely restates the outer context adds nothing.
///
/// The summary is printed above the rendering, so a cause carrying the same
/// sentence would repeat it. With no other cause the fallback must produce no
/// second rendering at all, which is what keeps a single-line report single.
#[test]
fn a_cause_that_restates_the_summary_is_dropped() -> Result<()> {
    let error = anyhow::Error::msg("outer summary").context("outer summary");
    ensure!(
        distinct_causes(&error).is_empty(),
        "a cause equal to the outer message is not distinct"
    );
    ensure!(
        failure_report(&error, &theme_for(presentation_style(false, true))).is_none(),
        "a failure whose only cause repeats its summary gains no rendering"
    );
    Ok(())
}

/// A selected manifest diagnostic still wins over the ordinary fallback.
///
/// The manifest wrapper is the selected diagnostic, so its stable code and
/// explanation must appear. The fallback must not replace or duplicate that
/// rendering merely because the chain also carries an ordinary context.
#[test]
fn selected_manifest_diagnostics_win_over_the_fallback() -> Result<()> {
    let error = manifest_failure().context("an extra ordinary cause");
    let report = failure_report(&error, &theme_for(presentation_style(false, true)))
        .context("a manifest failure should report its diagnostic")?;
    ensure!(
        report.contains("netsuke::manifest::schema_version"),
        "the selected manifest diagnostic must still be rendered: {report}"
    );
    ensure!(
        report.contains("this Netsuke supports schema 1.0"),
        "the selected diagnostic's explanation must survive: {report}"
    );
    ensure!(
        report.matches("an extra ordinary cause").count() == 1,
        "the outer context is printed once above the diagnostic: {report}"
    );
    Ok(())
}

/// A runner diagnostic buried in a chain still outranks ordinary causes.
///
/// `select_diagnostic` finds a runner error through `anyhow` context, so the
/// fallback must not engage. The plain cause is not part of the report, which
/// is also how a bounded runner diagnostic stays bounded.
#[test]
fn runner_diagnostics_outrank_ordinary_causes() -> Result<()> {
    let runner_error = runner::RunnerError::ManifestNotFound {
        manifest_name: String::from("Netsukefile"),
        directory: String::from("the current directory"),
        path: std::path::PathBuf::from("/workspace/Netsukefile"),
        message: localization::message(keys::RUNNER_MANIFEST_NOT_FOUND)
            .with_arg("manifest_name", "Netsukefile")
            .with_arg("directory", "the current directory"),
        help: localization::message(keys::RUNNER_MANIFEST_NOT_FOUND_HELP),
    };
    let ordinary_cause = std::io::Error::other("ordinary cause beneath the runner error");
    let error = anyhow::Error::new(ordinary_cause).context(runner_error);
    let report = failure_report(&error, &theme_for(presentation_style(false, true)))
        .context("a runner failure should report its diagnostic")?;
    ensure!(
        report.contains("netsuke::runner::manifest_not_found"),
        "the runner diagnostic must win the selection: {report}"
    );
    ensure!(
        !report.contains("ordinary cause beneath the runner error"),
        "the unselected ordinary cause must not be appended: {report}"
    );
    Ok(())
}

/// The fallback walks a linked source chain, which is what `miette` renders.
///
/// A flat message containing every cause would pass the report assertions above
/// while losing the chain structure. Traversing `source` pins the contract the
/// graphical renderer consumes.
#[test]
fn cause_chain_links_each_message_through_source() -> Result<()> {
    let chain = CauseChain::from_messages(vec![
        String::from("first cause"),
        String::from("second cause"),
    ])
    .context("a non-empty message list should build a chain")?;
    ensure!(
        chain.to_string() == "first cause",
        "the head must render the first cause"
    );
    let second = chain
        .source()
        .context("the chain must expose its second link as a source")?;
    ensure!(
        second.to_string() == "second cause",
        "the source must render the second cause"
    );
    ensure!(
        second.source().is_none(),
        "the final link must terminate the chain"
    );
    Ok(())
}

/// An empty message list builds no chain, so no empty rendering is produced.
#[test]
fn an_empty_cause_list_builds_no_diagnostic() -> Result<()> {
    ensure!(
        CauseChain::from_messages(Vec::new()).is_none(),
        "an empty cause list must not produce a diagnostic"
    );
    Ok(())
}
