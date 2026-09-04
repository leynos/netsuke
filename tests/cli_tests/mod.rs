//! Unit tests for CLI argument parsing and validation.
//!
//! This module exercises the command-line interface defined in `netsuke::cli`.

mod command_schema;
mod config_discovery;
#[cfg(unix)]
mod config_precedence_ladder;
mod config_selection;
mod display_policy_domain;
#[cfg(unix)]
mod fetch_policy_trust;
mod helpers;
mod locale;
mod merge;
mod merge_diag;
mod merge_diag_proptests;
mod merge_logging;
mod merge_observer;
mod merge_precedence_proptests;
mod merge_probe;
mod merge_targets_proptests;
mod parsing;
mod policy;
