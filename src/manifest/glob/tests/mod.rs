//! Tests for glob validation and expansion helpers.
//!
//! Split by concern: [`pattern`] covers normalisation and brace validation,
//! [`expansion`] covers the matches [`super::glob_paths`] returns,
//! [`capability`] covers the capability handle the metadata checks run
//! through, and [`property`] covers the prefix and relativisation invariants
//! the fixed cases are examples of.

mod capability;
mod expansion;
mod pattern;
mod property;
