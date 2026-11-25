//! Test-only accounting for dependency entries examined during cycle detection.
//!
//! The counter lives on the test thread and is reset at the start of each
//! detection pass, so focused tests can prove that an edge with several output
//! aliases is scanned once rather than once per alias. Non-test builds compile
//! the recorder as a no-op so production code carries no accounting state.

#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static DEPENDENCY_SCANS: Cell<usize> = const { Cell::new(0) };
}

/// Count one dependency entry examined by the detector.
#[cfg(test)]
pub(super) fn record_dependency_scan() {
    DEPENDENCY_SCANS.with(|count| count.set(count.get() + 1));
}

/// Skip dependency-scan counting outside test builds.
#[cfg(not(test))]
pub(super) const fn record_dependency_scan() {}

/// Reset this thread's dependency-scan count.
#[cfg(test)]
pub(super) fn reset_dependency_scans() {
    DEPENDENCY_SCANS.with(|count| count.set(0));
}

/// Report how many dependency entries this thread has examined.
#[cfg(test)]
pub(super) fn dependency_scans() -> usize {
    DEPENDENCY_SCANS.with(Cell::get)
}