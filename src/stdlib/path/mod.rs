//! Entry point for stdlib path and file utilities.
//!
//! Wires up path and file filters and re-exports crate-private helpers for
//! registration from the stdlib coordinator.
mod bounded_read;
mod filters;
mod fs_utils;
mod hash_utils;
mod path_utils;
mod read_telemetry;

#[cfg(test)]
mod home_metrics_tests;
#[cfg(test)]
mod home_tests;
#[cfg(test)]
mod read_telemetry_tests;

pub(crate) use filters::{register_filters, register_query_filters};
pub(crate) use fs_utils::file_type_matches;
pub use read_telemetry::{FILE_READ_FILTER_VALUES, FILE_READ_OUTCOME_VALUES, FILE_READ_TOTAL};
