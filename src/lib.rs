//! Netsuke core library.
//!
//! This library provides the command line interface definitions and
//! helper functions for parsing `Netsukefile` manifests.

pub mod ast;
pub mod cli;
pub use cli::localization as cli_localization;
pub mod diagnostic_json;
pub(crate) mod diagnostics;
pub mod graph_view;
pub mod hasher;
pub mod hex;
mod host;
pub(crate) use host::matching as host_matching;
pub use host::pattern as host_pattern;
pub mod ir;
mod json_envelope;
#[cfg(feature = "lint")]
pub mod lint;
mod locale;
pub use locale::{catalogues as locale_catalogues, resolution as locale_resolution};
pub mod localization;
pub mod manifest;
pub mod ninja_gen;
mod output;
pub use output::{mode as output_mode, prefs as output_prefs};
pub mod recipe_shell;
mod result_json;
pub mod runner;
mod shell_word;
#[cfg(test)]
mod snapshot_test_support;
pub mod status;
pub mod stdlib;
#[cfg(test)]
pub(crate) mod test_tracing_capture;
pub mod theme;
