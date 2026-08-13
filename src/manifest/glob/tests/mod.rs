//! Tests for glob validation and expansion helpers.
//!
//! Split by concern: [`pattern`] covers normalisation and brace validation,
//! [`expansion`] covers the matches [`super::glob_paths`] returns,
//! [`capability`] covers the capability handle the metadata checks run
//! through, [`diagnostics`] covers the counters and events it records, and
//! [`property`] covers the prefix and relativisation invariants the fixed
//! cases are examples of.

mod capability;
mod diagnostics;
mod expansion;
mod pattern;
mod property;
