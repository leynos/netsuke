//! Span and event cases for the bounded `which` resolver telemetry.
//!
//! A counter series says which search policy a resolution was requested under;
//! it cannot say what else travelled with it. These cases install a temporary
//! subscriber, resolve a fixture that is guaranteed to miss, and then read the
//! span fields and the failure event back. The claim under test is the
//! redaction one: the mode and the bounded outcome are the *whole* of what is
//! recorded — asserted as an exact field set rather than as the presence of the
//! fields expected — and nothing that would name the command, the workspace
//! root, or a searched directory is.

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use rstest::rstest;
use tracing::level_filters::LevelFilter;

use super::super::{
    options::CwdMode,
    telemetry::{CACHE_OUTCOME_MISS, CATEGORY_NOT_FOUND, RESOLUTION_OUTCOME_NOT_FOUND},
};
use super::{FIXTURE_PATHEXT, RESOLVER_SPAN, Workspace, options, path_override};
use crate::test_tracing_capture::with_test_subscriber;

/// The only search directory the miss case is handed, and it holds nothing.
///
/// The fixture is given this as its whole `PATH`: deterministic, distinctive,
/// and deliberately outside the workspace root. An entry nested inside the root
/// could not tell a leaked search directory from a leaked root, and the root is
/// excluded on its own account. Nothing creates the directory — a search entry
/// that does not exist is not an error, it simply yields no candidate — so the
/// lookup misses under every search policy.
const SEARCHED_DIR: &str = "/netsuke/which/telemetry/never-searched";

/// The message the failure event carries.
const FAILURE_MESSAGE: &str = "which resolver finished with non-success result";

/// The span fields the miss case must record, and no others.
///
/// Sorted, because the capture layer appends in recording order while the
/// comparison below is about the set: pinning the whole set is what makes a
/// field the resolver was never meant to record a failure, where looking for
/// the expected fields inside whatever was captured would never find one.
fn expected_span_fields(expected: &str) -> Vec<String> {
    let mut fields = vec![
        format!("cache_outcome={CACHE_OUTCOME_MISS:?}"),
        format!("cwd_mode={expected:?}"),
        format!("error_category={CATEGORY_NOT_FOUND:?}"),
        format!("result={RESOLUTION_OUTCOME_NOT_FOUND:?}"),
    ];
    fields.sort_unstable();
    fields
}

/// The bounded fields the failure event must carry, and no others.
///
/// The message is not among them: it renders as the event's own `message`
/// field, and the caller asserts it separately as the one thing left over.
fn expected_event_fields(expected: &str) -> Vec<String> {
    let mut fields = vec![
        format!("cwd_mode={expected:?}"),
        format!("outcome={RESOLUTION_OUTCOME_NOT_FOUND:?}"),
        format!("error_category={CATEGORY_NOT_FOUND:?}"),
    ];
    fields.sort_unstable();
    fields
}

/// Whether `event` carries exactly the bounded fields and the failure message.
///
/// The rendered fields of one event are compared as a set rather than as a
/// string: `tracing` visits the event's declared fields in declaration order,
/// which the message shares with the fields the resolver names, and that order
/// is the macro's to decide rather than this test's to encode. Each expected
/// field is removed once, and what remains must be the message alone — so a
/// field the resolver was never meant to record has nowhere to hide, while a
/// reordering of the fields the resolver *does* record is not a failure.
fn carries_only_bounded_fields(event: &str, expected: &str) -> Result<()> {
    let mut remainder = event.to_owned();
    for field in expected_event_fields(expected) {
        ensure!(
            remainder.contains(&field),
            "{expected}: the event should carry {field}: {event}"
        );
        remainder = remainder.replacen(&field, "", 1);
    }
    ensure!(
        remainder.trim() == format!("message={FAILURE_MESSAGE}"),
        "{expected}: the event should carry nothing but the bounded fields and \
         the message, but left {remainder:?}: {event}"
    );
    Ok(())
}

/// The span and the failure event carry the mode and bounded facts alone.
///
/// The command, the workspace root, the searched directory, and the pinned
/// extension list are the facts that must never be recorded: the first two
/// would name what a manifest asked for or where it looked, and the last two
/// would name where it looked next and what it would have accepted there.
/// Each is asserted absent from every captured event and span field rather than
/// merely unasserted, and the two subjects are pinned as exact sets so that a
/// field added to either cannot pass.
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
    let resolver = workspace.resolver(Some(path_override(&[Utf8Path::new(SEARCHED_DIR)])?))?;

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

    ensure!(
        missed,
        "{expected}: a search directory holding no fixture must miss"
    );

    let mut recorded = span.clone();
    recorded.sort_unstable();
    ensure!(
        recorded == expected_span_fields(expected),
        "{expected}: the span must record the bounded fields and nothing else: {span:?}"
    );

    let failure: Vec<&String> = events
        .iter()
        .filter(|event| event.contains(FAILURE_MESSAGE))
        .collect();
    ensure!(
        failure.len() == 1,
        "{expected}: expected one failure event but captured {failure:?}"
    );
    let event = failure
        .first()
        .copied()
        .context("the length check above leaves one failure event")?;
    carries_only_bounded_fields(event, expected)?;

    // The exact comparisons above already exclude these from the two subjects
    // they cover. Asserting them here as well states which values are sensitive
    // and covers anything captured outside those subjects, so a field added on
    // some other path is still held to the same redaction claim.
    for captured in events.iter().chain(span.iter()) {
        for leaked in [
            workspace.command.as_str(),
            workspace.root.as_str(),
            SEARCHED_DIR,
            FIXTURE_PATHEXT,
        ] {
            ensure!(
                !captured.contains(leaked),
                "{expected}: no captured field may name {leaked}: {captured}"
            );
        }
    }
    Ok(())
}
