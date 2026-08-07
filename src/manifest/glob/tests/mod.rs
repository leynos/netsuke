//! Tests for glob validation and expansion helpers.
//!
//! Split by concern: [`pattern`] covers normalisation and brace validation,
//! [`expansion`] covers the matches [`super::glob_paths`] returns, and
//! [`capability`] covers the capability handle the metadata checks run
//! through.

mod capability;
mod expansion;
mod pattern;
