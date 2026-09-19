//! Span and event cases for the bounded `which` resolver telemetry.
//!
//! A counter series says which domain a resolution used; it cannot say what
//! else travelled with it. These cases install a temporary subscriber,
//! resolve a fixture that is guaranteed to miss, and then read the span fields
//! and the failure event back. The claim under test is the redaction one: the
//! mode and the bounded outcome are present, and nothing that would name the
//! command or the workspace root is.

use std::ffi::OsString;

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use tracing::level_filters::LevelFilter;

use super::super::{
    options::CwdMode,
    telemetry::{CACHE_OUTCOME_MISS, CATEGORY_NOT_FOUND, RESOLUTION_OUTCOME_NOT_FOUND},
};
use super::{RESOLVER_SPAN, Workspace, options};
use crate::test_tracing_capture::with_test_subscriber;

/// The span and the failure event carry the mode and bounded facts alone.
///
/// The command and the workspace root are the facts that must never be
/// recorded: either would name what a manifest asked for or where it looked,
/// so both are asserted absent from every captured event and span field rather
/// than merely unasserted.
#[rstest]
#[case::auto(CwdMode::Auto, "auto")]
#[case::always(CwdMode::Always, "always")]
#[case::never(CwdMode::Never, "never")]
#[case::workspace_recursive(CwdMode::WorkspaceRecursive, "workspace_recursive")]
fn the_span_and_event_carry_the_mode_and_nothing_else(
    #[case] mode: CwdMode,
    #[case] expected: &str,
) -> Result<()> {
    let workspace = Workspace::new()?;
    let resolver = workspace.resolver(Some(OsString::new()))?;

    let (missed, events, span) = with_test_subscriber(LevelFilter::TRACE, |captured| {
        let missed = resolver
            .resolve(&workspace.command, &options(mode))
            .is_err();
        (
            missed,
            captured.snapshot(),
            captured.span_fields(RESOLVER_SPAN),
        )
    });

    ensure!(missed, "{expected}: the fixture should miss");
    for recorded in [
        format!("cwd_mode={expected:?}"),
        format!("cache_outcome={CACHE_OUTCOME_MISS:?}"),
        format!("result={RESOLUTION_OUTCOME_NOT_FOUND:?}"),
        format!("error_category={CATEGORY_NOT_FOUND:?}"),
    ] {
        ensure!(
            span.contains(&recorded),
            "{expected}: the span should record {recorded}: {span:?}"
        );
    }
    let failure: Vec<&String> = events
        .iter()
        .filter(|event| event.contains("which resolver finished with non-success result"))
        .collect();
    ensure!(
        failure.len() == 1,
        "{expected}: expected one failure event but captured {failure:?}"
    );
    let event = failure
        .first()
        .copied()
        .context("the length check above leaves one event")?;
    for recorded in [
        format!("cwd_mode={expected:?}"),
        format!("outcome={RESOLUTION_OUTCOME_NOT_FOUND:?}"),
        format!("error_category={CATEGORY_NOT_FOUND:?}"),
    ] {
        ensure!(
            event.contains(&recorded),
            "{expected}: the event should carry {recorded}: {event}"
        );
    }
    for captured in events.iter().chain(span.iter()) {
        ensure!(
            !captured.contains(&workspace.command) && !captured.contains(workspace.root.as_str()),
            "{expected}: no captured field may name the command or the root: {captured}"
        );
    }
    Ok(())
}
