//! Command-line parsing plus layered CLI configuration support.
//!
//! The parser-facing [`Cli`] type remains responsible for user-facing command
//! syntax, while [`CliConfig`] is the authoritative OrthoConfig-derived schema
//! used to merge defaults, configuration files, environment variables, and CLI
//! overrides into the runtime shape consumed by the runner.
//!
//! The module is split so that `build.rs` can compile the Clap schema alone.
//! `command`, [`config`], and `validation` form that self-contained slice;
//! every other submodule here is library-only. See the note in `build.rs`.

mod command;
pub mod config;
mod diag;
mod discovery;
mod merge;
mod parser;
mod parsing;
mod preferences;
mod validation;

#[cfg(test)]
pub(crate) mod test_support;

pub use command::{BuildArgs, Cli, Commands, GraphArgs};
pub use config::{AccessibilityPolicy, CliConfig, ColourPolicy, EmojiPolicy, ProgressPolicy};
pub use diag::{resolve_merged_json, resolve_merged_json_with_env};
pub use discovery::{EnvProvider as ConfigEnvProvider, StdEnvProvider as ConfigStdEnvProvider};
pub use merge::merge_with_config;
pub use parser::{json_hint_from_args, locale_hint_from_args, parse_with_localizer_from};
