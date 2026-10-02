//! Bounded tracing emitters for the path filters.
//!
//! Each emitter owns exactly one `tracing` macro so the caller's cognitive
//! complexity stays structural: the `log` feature, enabled transitively by a
//! dev-dependency, expands every tracing macro into extra branches. The
//! measurement and its rationale live in the migration ExecPlan; the rule for
//! new code is that a `tracing` macro invoked from a function under the
//! complexity threshold belongs in an emitter like these.
//!
//! The split also keeps `path_utils.rs` within the module line cap.
//!
//! Only bounding labels are ever emitted — never a resolved home, an
//! environment value, or a variable's contents.

use super::{EXPANDUSER_HOME_EVENT, HOME_OUTCOME_UNAVAILABLE};

/// Emit the bounded outcome of one `expanduser` home resolution.
pub(super) fn debug_home_resolved_from_fields(source: &'static str, found: bool) {
    tracing::debug!(
        event = EXPANDUSER_HOME_EVENT,
        source,
        found,
        "resolved the home directory for expanduser",
    );
}

/// Emit the bounded failure when no source supplied a home directory.
pub(super) fn debug_home_unavailable_from_fields(source: &'static str) {
    tracing::debug!(
        event = EXPANDUSER_HOME_EVENT,
        source,
        outcome = HOME_OUTCOME_UNAVAILABLE,
        "expanduser found no home directory",
    );
}
