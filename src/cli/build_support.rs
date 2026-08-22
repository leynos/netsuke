//! Build-script composition root for the CLI parser.
//!
//! The release manual needs Clap metadata from [`Cli`], but not runtime
//! configuration discovery or command merging. Keeping this subset separate
//! prevents the build script from compiling those runtime-only boundaries.

use ortho_config::OrthoError;
use std::sync::Arc;

mod config;
mod help;
mod parser;
mod parsing;
mod policy_values;
mod value_parser;

pub use config::{AccessibilityPolicy, CliConfig, ColourPolicy, EmojiPolicy, ProgressPolicy};
pub use parser::Cli;

/// Maximum number of jobs accepted by the CLI.
pub(super) const MAX_JOBS: usize = 64;

/// Build a validation `OrthoError` with the given key and message.
pub(super) fn validation_error(key: &str, message: &str) -> Arc<OrthoError> {
    Arc::new(OrthoError::Validation {
        key: key.to_owned(),
        message: message.to_owned(),
    })
}
