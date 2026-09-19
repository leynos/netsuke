//! Bounded telemetry for the `which` resolver.
//!
//! The resolver has four search domains and one cache, and before this module
//! its counters could not say which domain a resolution used. A
//! `workspace-recursive` miss and an `auto` miss produced the same series, so
//! an operator could not tell whether recursive lookup contributed to a
//! resolution, nor whether a manifest had requested it at all.
//!
//! Two counters are owned here. `netsuke_stdlib_which_cache_total` counts
//! cache outcomes, and `netsuke_stdlib_which_resolution_total` counts
//! resolution outcomes. Both carry a `cwd_mode` label drawn from the closed
//! [`WHICH_CWD_MODE_VALUES`] set. That set is a telemetry vocabulary rather
//! than the template spelling: a manifest writes `workspace-recursive`, and
//! the label is `workspace_recursive`.
//!
//! Every label is drawn from a closed set declared in this module, and nothing
//! else is recorded: no command name, no filesystem path, no workspace name,
//! and no `PATH` or `PATHEXT` value. A series can therefore be exported
//! without disclosing what a manifest asked for or where it was found.

use std::sync::Once;

use metrics::{counter, describe_counter};

use super::{options::CwdMode, resolve_error::ResolveError};

/// Counts resolver cache outcomes by bounded `cwd_mode` and `outcome`.
///
/// Both labels are drawn from the closed sets below, so the number of series
/// is fixed by this module rather than by anything a template supplies.
pub const WHICH_CACHE_TOTAL: &str = "netsuke_stdlib_which_cache_total";

/// Counts resolver outcomes by bounded `cwd_mode` and `outcome`.
///
/// A non-success outcome also carries the bounded `category` label, so the
/// counter's series take one of two label shapes. Both are declared here, and
/// the application recorder admits each by its exact shape.
pub const WHICH_RESOLUTION_TOTAL: &str = "netsuke_stdlib_which_resolution_total";

/// The bounded `cwd_mode` recorded when the search domain is `auto`.
const CWD_MODE_AUTO: &str = "auto";
/// The bounded `cwd_mode` recorded when the search domain is `always`.
const CWD_MODE_ALWAYS: &str = "always";
/// The bounded `cwd_mode` recorded when the search domain is `never`.
const CWD_MODE_NEVER: &str = "never";
/// The bounded `cwd_mode` recorded when the search domain is
/// `workspace-recursive`.
const CWD_MODE_WORKSPACE_RECURSIVE: &str = "workspace_recursive";

/// The closed `cwd_mode` vocabulary admitted on both resolver counters.
///
/// Four values, one per [`CwdMode`] variant. The label set is what lets an
/// operator attribute a resolution to a search domain; it is not the template
/// spelling, which uses a hyphen for the recursive mode.
pub const WHICH_CWD_MODE_VALUES: [&str; 4] = [
    CWD_MODE_AUTO,
    CWD_MODE_ALWAYS,
    CWD_MODE_NEVER,
    CWD_MODE_WORKSPACE_RECURSIVE,
];

/// The bounded `outcome` recorded when the resolver answered from its cache.
pub(super) const CACHE_OUTCOME_HIT: &str = "hit";
/// The bounded `outcome` recorded when the cache held no entry for the key.
pub(super) const CACHE_OUTCOME_MISS: &str = "miss";
/// The bounded `outcome` recorded when the caller bypassed the cache.
pub(super) const CACHE_OUTCOME_BYPASS: &str = "bypass";

/// The closed `outcome` vocabulary admitted on [`WHICH_CACHE_TOTAL`].
pub const WHICH_CACHE_OUTCOME_VALUES: [&str; 3] =
    [CACHE_OUTCOME_HIT, CACHE_OUTCOME_MISS, CACHE_OUTCOME_BYPASS];

/// The bounded `outcome` recorded when a resolution produced matches.
pub(super) const RESOLUTION_OUTCOME_FOUND: &str = "found";
/// The bounded `outcome` recorded when no executable was discovered.
pub(super) const RESOLUTION_OUTCOME_NOT_FOUND: &str = "not_found";
/// The bounded `outcome` recorded when the resolution failed for another reason.
pub(super) const RESOLUTION_OUTCOME_ERROR: &str = "error";

/// The closed `outcome` vocabulary admitted on [`WHICH_RESOLUTION_TOTAL`].
pub const WHICH_RESOLUTION_OUTCOME_VALUES: [&str; 3] = [
    RESOLUTION_OUTCOME_FOUND,
    RESOLUTION_OUTCOME_NOT_FOUND,
    RESOLUTION_OUTCOME_ERROR,
];

/// The outcomes that carry an `error_category`, and so take three labels.
///
/// A resolution records a category only when it fails, so the three-label
/// series are exactly these two outcomes. Declaring the subset lets the
/// application recorder admit each label shape precisely — a `found` series
/// carrying a category is a bug elsewhere, not telemetry to export.
pub const WHICH_RESOLUTION_FAILURE_OUTCOME_VALUES: [&str; 2] =
    [RESOLUTION_OUTCOME_NOT_FOUND, RESOLUTION_OUTCOME_ERROR];

/// The bounded `category` recorded for a PATH search miss.
pub const CATEGORY_NOT_FOUND: &str = "not_found";
/// The bounded `category` recorded for a direct-path lookup miss.
pub const CATEGORY_DIRECT_NOT_FOUND: &str = "direct_not_found";
/// The bounded `category` recorded for an invalid argument or option value.
pub const CATEGORY_ARGS: &str = "args";
/// The bounded `category` recorded when canonicalization failed.
pub const CATEGORY_CANONICALIZE: &str = "canonicalize";
/// The bounded `category` recorded when an executable probe failed.
pub const CATEGORY_IS_EXECUTABLE: &str = "is_executable";
/// The bounded `category` recorded for a non-UTF-8 canonical path.
pub const CATEGORY_CANONICALIZE_NON_UTF8: &str = "canonicalize_non_utf8";
/// The bounded `category` recorded for a non-UTF-8 workspace path.
pub const CATEGORY_WORKSPACE_NON_UTF8: &str = "workspace_non_utf8";
/// The bounded `category` recorded for a workspace traversal failure.
pub const CATEGORY_WALKDIR: &str = "walkdir";
/// The bounded `category` recorded when the working directory could not be read.
pub const CATEGORY_CWD_RESOLVE: &str = "cwd_resolve";
/// The bounded `category` recorded for a non-UTF-8 working directory.
pub const CATEGORY_CWD_NON_UTF8: &str = "cwd_non_utf8";

/// The closed `category` vocabulary admitted on [`WHICH_RESOLUTION_TOTAL`].
///
/// One value per [`ResolveError`] variant, so the label set is fixed by the
/// error type rather than by the failure a host happened to encounter.
pub const RESOLVE_ERROR_CATEGORY_VALUES: [&str; 10] = [
    CATEGORY_NOT_FOUND,
    CATEGORY_DIRECT_NOT_FOUND,
    CATEGORY_ARGS,
    CATEGORY_CANONICALIZE,
    CATEGORY_IS_EXECUTABLE,
    CATEGORY_CANONICALIZE_NON_UTF8,
    CATEGORY_WORKSPACE_NON_UTF8,
    CATEGORY_WALKDIR,
    CATEGORY_CWD_RESOLVE,
    CATEGORY_CWD_NON_UTF8,
];

/// Return the bounded `cwd_mode` label for a search domain.
///
/// The mapping is total over [`CwdMode`], so every resolution carries a label
/// from [`WHICH_CWD_MODE_VALUES`] and no series can be created outside it.
pub(super) const fn cwd_mode_label(mode: CwdMode) -> &'static str {
    match mode {
        CwdMode::Auto => CWD_MODE_AUTO,
        CwdMode::Always => CWD_MODE_ALWAYS,
        CwdMode::Never => CWD_MODE_NEVER,
        CwdMode::WorkspaceRecursive => CWD_MODE_WORKSPACE_RECURSIVE,
    }
}

/// Describe the resolver's counters once per process.
fn describe_which_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            WHICH_CACHE_TOTAL,
            "Counts which resolver cache outcomes labelled by cwd_mode (auto, \
             always, never, or workspace_recursive) and by outcome (hit, miss, \
             or bypass)."
        );
        describe_counter!(
            WHICH_RESOLUTION_TOTAL,
            "Counts which resolver outcomes labelled by cwd_mode (auto, always, \
             never, or workspace_recursive) and by outcome (found, not_found, \
             or error); non-success outcomes also carry a bounded category."
        );
    });
}

/// Record one cache outcome on the span and its counter.
///
/// `cwd_mode` is a label from [`WHICH_CWD_MODE_VALUES`] and `outcome` one from
/// [`WHICH_CACHE_OUTCOME_VALUES`]; neither is derived from manifest content.
pub(super) fn record_cache_outcome(
    span: &tracing::Span,
    cwd_mode: &'static str,
    outcome: &'static str,
) {
    describe_which_metrics();
    span.record("cache_outcome", outcome);
    counter!(
        WHICH_CACHE_TOTAL,
        "cwd_mode" => cwd_mode,
        "outcome" => outcome,
    )
    .increment(1);
}

/// Record a successful resolution on the span and its counter.
pub(super) fn record_resolution_found(span: &tracing::Span, cwd_mode: &'static str) {
    describe_which_metrics();
    span.record("result", RESOLUTION_OUTCOME_FOUND);
    counter!(
        WHICH_RESOLUTION_TOTAL,
        "cwd_mode" => cwd_mode,
        "outcome" => RESOLUTION_OUTCOME_FOUND,
    )
    .increment(1);
}

/// Record a resolution failure's outcome and bounded error category as metrics.
///
/// A search or direct-path miss is counted as `not_found` and every other
/// failure as `error`, so the two categories an operator acts on — nothing was
/// found, versus something went wrong — stay separable. The debug event
/// repeats the bounded facts for a reader of the log alone; neither it nor the
/// counter names the command that failed.
pub(super) fn record_resolution_error(
    span: &tracing::Span,
    cwd_mode: &'static str,
    error: &ResolveError,
) {
    describe_which_metrics();
    let category = error.category();
    let outcome = if matches!(
        error,
        ResolveError::NotFound { .. } | ResolveError::DirectNotFound { .. }
    ) {
        RESOLUTION_OUTCOME_NOT_FOUND
    } else {
        RESOLUTION_OUTCOME_ERROR
    };
    span.record("result", outcome);
    span.record("error_category", category);
    tracing::debug!(
        cwd_mode,
        outcome,
        error_category = category,
        "which resolver finished with non-success result",
    );
    counter!(
        WHICH_RESOLUTION_TOTAL,
        "cwd_mode" => cwd_mode,
        "outcome" => outcome,
        "category" => category,
    )
    .increment(1);
}
