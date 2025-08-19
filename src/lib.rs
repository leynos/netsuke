//! Netsuke core library.
//!
//! This library provides the command line interface definitions and
//! helper functions for parsing `Netsukefile` manifests.

pub mod ast;
pub mod cli;
pub mod diagnostics;
pub mod hasher;
pub mod ir;
pub mod manifest;
pub mod ninja_gen;
pub mod runner;
