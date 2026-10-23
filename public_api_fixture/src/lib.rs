//! Compile Netsuke's embedder-facing API as an external crate would.
//!
//! Each module type-checks one public boundary from outside the `netsuke`
//! crate, so a visibility or signature regression fails the ordinary workspace
//! build rather than a timed test. Nothing here runs: the functions exist to be
//! compiled, and Cargo compiles them in the build phase with every other
//! workspace library. See ADR-0059 for why the checks left the test suite.

pub mod cli_configuration;
pub mod command_env;
pub mod command_list;
pub mod config_cached_discovery;
pub mod ir_gen_error;
pub mod legacy_recipe_telemetry;
#[cfg(feature = "lint")]
pub mod lint;
pub mod manifest_environment;
pub mod policy_from_str;
pub mod verbose_timing_reporter;
