//! Command-line parsing plus layered CLI configuration support.
//!
//! The parser-facing [`Cli`] type remains responsible for user-facing command
//! syntax, while [`CliConfig`] is the authoritative OrthoConfig-derived schema
//! used to merge defaults, configuration files, environment variables, and CLI
//! overrides into the runtime shape consumed by the runner.

use ortho_config::OrthoError;
use std::sync::Arc;

mod config;
mod merge;
mod parser;
mod parsing;

pub use config::{CliConfig, ColourPolicy, OutputFormat, SpinnerMode, Theme};
pub use merge::{merge_with_config, resolve_merged_diag_json};
pub use parser::{
    BuildArgs, Cli, Commands, diag_json_hint_from_args, locale_hint_from_args,
    parse_with_localizer_from,
};

pub(super) fn validation_error(key: &str, message: &str) -> Arc<OrthoError> {
    Arc::new(OrthoError::Validation {
        key: key.to_owned(),
        message: message.to_owned(),
    })
}
