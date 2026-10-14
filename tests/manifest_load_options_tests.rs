//! Characterization and differential tests for manifest loader entry points.
//!
//! The module tree under `tests/manifest_load_options/` records what every
//! public loader entry point produces today, before the options-carrying entry
//! point replaces the loader's internals. The pre-refactor behaviour is the
//! only trustworthy oracle for the refactor's equivalence claim, so these
//! snapshots are recorded once and never re-accepted.
mod manifest_load_options;
