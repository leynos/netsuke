//! Admit the bounded `netsuke check` metric series.
//!
//! The command exists only with the `lint` feature, so its outcome vocabulary
//! and name predicate sit behind one gate here rather than among the
//! recorder's shared series, and a default-feature build admits none of them.

#[cfg(feature = "lint")]
use netsuke::runner::{CHECK_DURATION, CHECK_TOTAL};

/// Report whether `name` is one of `netsuke check`'s metrics.
#[cfg(feature = "lint")]
pub(super) fn is_check_metric(name: &str) -> bool {
    matches!(name, CHECK_TOTAL | CHECK_DURATION)
}

/// Report no `netsuke check` metrics: the command is not compiled in.
#[cfg(not(feature = "lint"))]
pub(super) const fn is_check_metric(_name: &str) -> bool {
    false
}

/// Bounded command outcomes emitted by `netsuke check`.
#[cfg(feature = "lint")]
pub(super) const CHECK_OUTCOMES: [&str; 5] = [
    "success",
    "threshold_failure",
    "policy_failure",
    "analysis_failure",
    "output_failure",
];
