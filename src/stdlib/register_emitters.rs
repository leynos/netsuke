//! Bounded tracing emitters for stdlib registration.
//!
//! Each emitter owns exactly one `tracing` macro so the caller's cognitive
//! complexity stays structural: the `log` feature, enabled transitively by a
//! dev-dependency, expands every tracing macro into extra branches. The
//! measurement and its rationale live in the migration ExecPlan; the rule for
//! new code is that a `tracing` macro invoked from a function under the
//! complexity threshold belongs in an emitter like these.
//!
//! The split also keeps `register.rs` within the module line cap.

/// Note the clock source backing the registered stdlib time helpers.
pub(super) fn debug_time_helpers_registered_from_fields(clock_source: &str) {
    tracing::debug!(clock_source, "registered stdlib time helpers");
}

/// Note the byte budget the registered stdlib file filters enforce.
pub(super) fn debug_file_filters_registered_from_fields(file_max_read_bytes: u64) {
    tracing::debug!(
        file_max_read_bytes,
        "registered stdlib file-reading filters"
    );
}
