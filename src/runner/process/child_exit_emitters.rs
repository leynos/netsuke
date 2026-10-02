//! Bounded tracing emitters for child-process shutdown.
//!
//! Each emitter owns exactly one `tracing` macro so the caller's cognitive
//! complexity stays structural: the `log` feature, enabled transitively by a
//! dev-dependency, expands every tracing macro into extra branches. The
//! measurement and its rationale live in the migration ExecPlan; the rule for
//! new code is that a `tracing` macro invoked from a function under the
//! complexity threshold belongs in an emitter like these.
//!
//! The split also keeps `child_exit.rs` from concentrating string-taking
//! functions, which CodeScene's *String Heavy Function Arguments* rule reads as
//! a file-level ratio rather than a per-function verdict. Four small emitters
//! added to an otherwise numeric module pushed that ratio from 2/5 to 5/9 and
//! dropped the file from 10.00 to 9.68; siting them here keeps every doc-comment
//! emitter in the module whose subject is tracing.

use std::io;

/// Note that a partially configured child could not be terminated.
pub(super) fn debug_child_kill_failed(context: &str, error: &io::Error) {
    tracing::debug!("failed to kill child after {context}: {error}");
}

/// Note that a terminated child could not be reaped.
pub(super) fn debug_child_reap_failed(context: &str, error: &io::Error) {
    tracing::debug!("failed to reap child after {context}: {error}");
}

/// Note that a forwarding thread panicked and its stats were discarded.
pub(super) fn warn_forwarding_thread_panicked(panic: &dyn std::fmt::Debug) {
    tracing::warn!("stderr forwarding thread panicked: {panic:?}");
}

/// Note that a forwarding stream hit a closed pipe and truncated its output.
pub(super) fn debug_forwarding_stream_truncated(stream_name: &str) {
    tracing::debug!("{stream_name} forwarding encountered closed pipe; output truncated");
}
