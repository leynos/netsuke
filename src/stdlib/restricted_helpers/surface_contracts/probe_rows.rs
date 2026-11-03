//! Every inventory row, exercised through a real manifest load.
//!
//! Split from the single-file contracts module, which outgrew the module line
//! cap, along the claim it serves: this file is the half that needs a loader,
//! while `registration_scan.rs` reads the source text and `refusal_text.rs`
//! pins the refusal shape. The rows are checked together so a single failure report
//! names every row that drifted rather than only the first.

use minijinja::{Error as MiniJinjaError, ErrorKind};

use super::super::{HelperEntry, SURFACE, TestDisposition};
use super::{jinja_error, load_probe, probe_expression, refusal_operation};

/// Check one inventory row against the probe's outcome.
fn check_row(entry: &HelperEntry, expression: &str) -> std::result::Result<(), String> {
    let outcome = load_probe(expression);
    match entry.test {
        TestDisposition::Available => outcome
            .map(|_| ())
            .map_err(|error| format!("expected `{expression}` to load, got {error:#}")),
        TestDisposition::Refused | TestDisposition::ClockInjected => match outcome {
            Ok(_) => Err(format!("expected `{expression}` to refuse, but it loaded")),
            Err(error) => check_refusal(entry, &error),
        },
    }
}

/// Report whether `kind` is `MiniJinja`'s unknown-helper error.
///
/// An inventory row whose helper was never registered resolves as unknown
/// rather than refused, which is a different fault from a misnamed refusal and
/// is reported with its own message.
fn is_unknown_helper(kind: ErrorKind) -> bool {
    matches!(
        kind,
        ErrorKind::UnknownFunction | ErrorKind::UnknownFilter | ErrorKind::UnknownTest
    )
}

/// Check a refused probe names its helper and is not an unknown-helper error.
fn check_refusal(entry: &HelperEntry, error: &anyhow::Error) -> std::result::Result<(), String> {
    if let Some(kind) = jinja_error(error)
        .map(MiniJinjaError::kind)
        .filter(|kind| is_unknown_helper(*kind))
    {
        return Err(format!(
            "`{}` resolved as an unknown helper: {kind:?}",
            entry.name
        ));
    }
    match refusal_operation(error) {
        Some(operation) if operation == entry.name => Ok(()),
        Some(operation) => Err(format!("`{}` refused as `{operation}`", entry.name)),
        None => Err(format!(
            "`{}` failed without a typed refusal: {error:#}",
            entry.name
        )),
    }
}

/// Assert every inventory row can be exercised and answers as recorded.
#[test]
fn every_inventory_row_has_a_reachable_probe() {
    let mut failures = Vec::new();
    for entry in SURFACE {
        match probe_expression(entry) {
            Some(expression) => {
                if let Err(failure) = check_row(entry, expression) {
                    failures.push(format!("{} ({:?}): {failure}", entry.name, entry.kind));
                }
            }
            None => failures.push(format!(
                "{} ({:?}): no probe expression is defined",
                entry.name, entry.kind
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} inventory rows failed:\n{}",
        failures.len(),
        SURFACE.len(),
        failures.join("\n")
    );
}
