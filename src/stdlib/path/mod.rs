//! Entry point for stdlib path and file utilities.
//!
//! Wires up path and file filters and re-exports crate-private helpers for
//! registration from the stdlib coordinator.
mod filters;
mod fs_utils;
mod hash_utils;
mod path_utils;

pub(crate) use filters::register_filters;
pub(crate) use fs_utils::file_type_matches;
