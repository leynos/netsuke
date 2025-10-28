//! Netsuke core library.
//!
//! This library provides the command line interface definitions and
//! helper functions for parsing `Netsukefile` manifests.

pub mod ast;
pub mod cli;
mod cli_policy;
pub(crate) mod diagnostics;
pub mod hasher;
pub mod host_pattern;
pub mod ir;
pub mod manifest;
pub mod ninja_gen;
pub mod runner;
pub mod stdlib;
