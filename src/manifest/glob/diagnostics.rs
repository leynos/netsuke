//! Bounded observability for glob expansion.
//!
//! Two outcomes of the capability-scoped walk are expected rather than
//! erroneous, so neither reaches the top-level diagnostics: a literal prefix
//! that names no directory, and a match that the capability cannot resolve
//! because a symbolic link escapes the prefix. Both are recorded here so a
//! degraded expansion is visible without having to reproduce it.
//!
//! What is recorded is deliberately bounded, but not uniformly redacted.
//!
//! Metric labels carry only a closed set of outcome and reason strings, never
//! the pattern or a path, in line with the low-cardinality rule in `AGENTS.md`.
//!
//! Tracing events preserve relative patterns and prefixes, but replace either
//! absolute form with the stable `<absolute>` marker. Errors still retain the
//! caller's pattern so they can explain invalid input precisely; tracing does
//! not need that detail to identify the expansion outcome.
//!
//! What tracing does not carry is a matched path. A skipped entry is recorded
//! relative to the literal prefix, so the event names only what the pattern
//! itself already reached for and never discloses where that prefix sits on
//! disk. This is looser than the `path_hash` convention used for
//! configuration discovery, where the paths are ones the tool found rather
//! than ones the user named.

use super::GlobPattern;
use camino::Utf8Path;
use metrics::{counter, describe_counter};
use std::sync::Once;

const EXPANSIONS_TOTAL: &str = "netsuke_manifest_glob_expansions_total";
const ENTRIES_SKIPPED_TOTAL: &str = "netsuke_manifest_glob_entries_skipped_total";
const ABSOLUTE_PATH: &str = "<absolute>";

/// Return a useful trace value without disclosing an absolute path.
fn bounded_path(value: &str) -> &str {
    if Utf8Path::new(value).is_absolute() {
        ABSOLUTE_PATH
    } else {
        value
    }
}

/// Register the metric descriptions once per process.
fn describe_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            EXPANSIONS_TOTAL,
            "Counts glob expansions labelled by outcome: matched, or \
             unopenable_prefix when the pattern's literal directory prefix \
             names no directory."
        );
        describe_counter!(
            ENTRIES_SKIPPED_TOTAL,
            "Counts matched entries dropped from a glob expansion, labelled \
             by reason: unreachable_symlink for a link the capability cannot \
             resolve, not_a_file for a directory or other non-file."
        );
    });
}

/// Record an expansion that stopped because the literal prefix is unusable.
pub(super) fn record_unopenable_prefix(pattern: &GlobPattern, prefix: &str) {
    describe_metrics();
    counter!(EXPANSIONS_TOTAL, "outcome" => "unopenable_prefix").increment(1);
    tracing::debug!(
        pattern = %bounded_path(pattern.raw()),
        prefix = %bounded_path(prefix),
        "glob literal prefix names no directory; expanding to no matches"
    );
}

/// Record an expansion that ran the walk to completion.
pub(super) fn record_expansion_matched(pattern: &GlobPattern, matches: usize) {
    describe_metrics();
    counter!(EXPANSIONS_TOTAL, "outcome" => "matched").increment(1);
    tracing::debug!(
        pattern = %bounded_path(pattern.raw()),
        matches,
        "glob expansion complete"
    );
}

/// Record a match dropped because the capability cannot resolve it.
///
/// `relative` is the match relative to the literal prefix, so it stays within
/// the scope the pattern already named.
pub(super) fn record_unreachable_symlink(relative: &Utf8Path) {
    describe_metrics();
    counter!(ENTRIES_SKIPPED_TOTAL, "reason" => "unreachable_symlink").increment(1);
    tracing::debug!(
        relative = %relative,
        "glob match traverses a symbolic link the capability cannot resolve; skipping"
    );
}

/// Record a match dropped because it does not name a regular file.
pub(super) fn record_not_a_file() {
    describe_metrics();
    counter!(ENTRIES_SKIPPED_TOTAL, "reason" => "not_a_file").increment(1);
}
