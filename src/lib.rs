//! Netsuke core library.
//!
//! This library provides the command line interface definitions and
//! helper functions for parsing `Netsukefile` manifests.

pub mod ast;
pub mod cli;
mod cli_l10n;
pub mod cli_localization;
mod cli_policy;
pub(crate) mod diagnostics;
pub mod hasher;
pub mod host_pattern;
pub mod ir;
pub mod locale_resolution;
pub mod localization;
pub mod manifest;
pub mod ninja_gen;
pub mod output_mode;
pub mod runner;
pub mod status;
pub mod stdlib;
