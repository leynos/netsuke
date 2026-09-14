//! Bounded telemetry for the file-reading filters.
//!
//! `contents`, `linecount`, `hash`, and `digest` share one read boundary, so
//! they share one telemetry point: each filter closure hands its result to
//! [`record_file_read`], which counts the call under a closed `filter` label
//! and a closed `outcome` label and emits one debug event carrying the
//! effective byte budget and the symlink policy the call ran under. A call
//! refused on its keywords resolves neither, so it is counted without them
//! rather than against a policy it never used.
//!
//! Both label sets are constants in this module, and nothing else is recorded:
//! a path, a file's contents, and a rendered value are absent by construction,
//! so the series can be exported without disclosing what a template read. The
//! rejection *category* is likewise not a label — it is already the localized
//! diagnostic the caller sees, and collapsing it into a bounded label would
//! either lose the distinction or grow the label set with the locale space.
use std::sync::Once;

use metrics::{counter, describe_counter};
use minijinja::Error;

use super::fs_utils::FileReadLimits;

/// The `event` field naming every file-read telemetry event.
pub(super) const FILE_READ_EVENT: &str = "stdlib.file_read.read";

/// Counts file-reading filter calls by bounded `filter` and `outcome`.
///
/// Both labels are drawn from the closed sets below, so the number of series
/// is fixed by this module rather than by anything a template supplies. The
/// application recorder admits the series through the same two sets, so the
/// counter is exported rather than silently dropped as a noop handle.
pub const FILE_READ_TOTAL: &str = "netsuke_stdlib_file_read_total";

/// The `contents` filter, which renders UTF-8 text.
pub(super) const FILTER_CONTENTS: &str = "contents";
/// The `linecount` filter, which counts the lines of UTF-8 text.
pub(super) const FILTER_LINECOUNT: &str = "linecount";
/// The `hash` filter, which renders a file's hex digest.
pub(super) const FILTER_HASH: &str = "hash";
/// The `digest` filter, which renders a prefix of a file's hex digest.
pub(super) const FILTER_DIGEST: &str = "digest";

/// The bounded `outcome` recorded when the filter produced a value.
pub(super) const OUTCOME_OK: &str = "ok";
/// The bounded `outcome` recorded when the filter rejected the call.
pub(super) const OUTCOME_REJECTED: &str = "rejected";

/// The closed `filter` vocabulary admitted on [`FILE_READ_TOTAL`].
pub const FILE_READ_FILTER_VALUES: [&str; 4] = [
    FILTER_CONTENTS,
    FILTER_LINECOUNT,
    FILTER_HASH,
    FILTER_DIGEST,
];

/// The closed `outcome` vocabulary admitted on [`FILE_READ_TOTAL`].
pub const FILE_READ_OUTCOME_VALUES: [&str; 2] = [OUTCOME_OK, OUTCOME_REJECTED];

/// Describe the file-read counter once per process.
fn describe_file_read_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            FILE_READ_TOTAL,
            "Counts calls to the file-reading filters labelled by filter \
             (contents, linecount, hash, or digest) and by outcome (ok or \
             rejected)."
        );
    });
}

/// Record one file-reading filter call and return its result unchanged.
///
/// This is the telemetry boundary for the four filters: every call is counted
/// exactly once, whatever the outcome, and the event names the budget and
/// symlink policy the call ran under so a rejection can be attributed without
/// the path ever leaving the process. `limits` is `None` only for a call
/// refused before its keywords resolved, which has no effective policy to
/// report.
pub(super) fn record_file_read<T>(
    filter: &'static str,
    limits: Option<&FileReadLimits>,
    result: Result<T, Error>,
) -> Result<T, Error> {
    describe_file_read_metrics();
    let outcome = if result.is_ok() {
        OUTCOME_OK
    } else {
        OUTCOME_REJECTED
    };
    if let Some(resolved) = limits {
        tracing::debug!(
            event = FILE_READ_EVENT,
            filter,
            outcome,
            limit = resolved.max_bytes,
            follow_symlinks = resolved.follow_symlinks,
            "read a file for a stdlib filter",
        );
    } else {
        tracing::debug!(
            event = FILE_READ_EVENT,
            filter,
            outcome,
            "a stdlib filter call was refused before its read limits resolved",
        );
    }
    counter!(
        FILE_READ_TOTAL,
        "filter" => filter,
        "outcome" => outcome,
    )
    .increment(1);
    result
}

/// Record one filter call refused before its keywords resolved.
///
/// An undeclared keyword or a malformed `max_bytes` stops the call before it
/// has a budget or a symlink policy, so [`record_file_read`] is handed no
/// limits to report. The refusal is still a filter outcome, and counting it
/// keeps the series a faithful tally of calls rather than of reads alone.
pub(super) fn record_unresolved_read<T>(filter: &'static str, err: Error) -> Result<T, Error> {
    record_file_read(filter, None, Err(err))
}
